#pragma once

#include <QByteArray>
#include <QObject>
#include <QString>
#include <QUrl>

#include <cstdint>
#include <functional>
#include <memory>
#include <optional>

class QNetworkAccessManager;
class QNetworkReply;

struct AgentResult
{
	bool ok = false;
	QString code;
	QString message;
	int http_status = 0;
};

struct ChatDetectionResult
{
	AgentResult result;
	qint64 chat_id = 0;
	QString challenge;
	bool confirmed = false;
};

struct CreatedJobResult
{
	AgentResult result;
	QString job_id;
	QString state;
};

struct JobStatusResult
{
	AgentResult result;
	QString job_id;
	QString filename;
	QString state;
	std::optional<int> progress_percent;
	QString message;
	bool retryable = false;
};

class AgentClient : public QObject
{
      public:
	using ResultHandler = std::function<void(AgentResult)>;
	using ChatHandler = std::function<void(ChatDetectionResult)>;
	using CreatedJobHandler = std::function<void(CreatedJobResult)>;
	using JobStatusHandler = std::function<void(JobStatusResult)>;

	AgentClient(QUrl base_url, QString bearer, bool configuration_known, QObject *parent = nullptr,
	            bool persist_configuration_state = false);
	static std::unique_ptr<AgentClient> from_system(QObject *parent = nullptr);

	bool is_ready() const;
	bool configuration_known() const;
	void probe(ResultHandler handler);

	void save_config(const QString &bot_token, std::uint32_t api_id, const QString &api_hash, qint64 chat_id,
	                 ResultHandler handler);
	void detect_chat(const QString &bot_token, std::uint32_t api_id, const QString &api_hash,
	                 const QString &challenge, ChatHandler handler);
	void test_send(ResultHandler handler);
	void create_job(const QString &recording_path, const QString &display_name, CreatedJobHandler handler);
	void get_job(const QString &job_id, JobStatusHandler handler);
	void retry_job(const QString &job_id, JobStatusHandler handler);

	AgentResult parse_error(const QByteArray &body, int http_status = 0) const;
	JobStatusResult parse_job_status(const QByteArray &body, int http_status = 200) const;

      private:
	using ReplyHandler = std::function<void(int, QByteArray)>;

	void send_request(const QByteArray &method, const QString &path, const QByteArray &body, bool authenticated,
	                  ReplyHandler handler);
	AgentResult parse_empty_success(const QByteArray &body, int http_status) const;
	QString safe_message(QString message) const;
	void forget_configuration_ready();
	void remember_configuration_ready();

	QUrl base_url_;
	QString bearer_;
	bool configuration_known_ = false;
	bool agent_available_ = false;
	bool persist_configuration_state_ = false;
	QNetworkAccessManager *network_ = nullptr;
};
