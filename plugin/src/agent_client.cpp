#include "agent_client.hpp"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkAccessManager>
#include <QNetworkProxy>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QSettings>
#include <QStandardPaths>

#include <algorithm>
#include <utility>

namespace {

constexpr auto settings_organization = "OBS Telegram Send";
constexpr auto settings_application = "OBS Telegram Send";
constexpr auto configured_key = "connection/configuration-tested";

bool successful_status(int status)
{
	return status >= 200 && status < 300;
}

QJsonObject object_from(const QByteArray &body)
{
	const auto document = QJsonDocument::fromJson(body);
	return document.isObject() ? document.object() : QJsonObject{};
}

AgentResult success_result(int status)
{
	return AgentResult{true, {}, {}, status};
}

} // namespace

AgentClient::AgentClient(QUrl base_url, QString bearer, bool configuration_known, QObject *parent,
                         bool persist_configuration_state)
    : QObject(parent), base_url_(std::move(base_url)), bearer_(std::move(bearer)),
      configuration_known_(configuration_known), persist_configuration_state_(persist_configuration_state),
      network_(new QNetworkAccessManager(this))
{
	base_url_.setPath({});
	network_->setProxy(QNetworkProxy::NoProxy);
}

std::unique_ptr<AgentClient> AgentClient::from_system(QObject *parent)
{
	const auto data_directory = QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation);
	QFile bearer_file(QDir(data_directory).filePath(QStringLiteral("OBS-Telegram-Send/loopback-install-bearer")));
	QString bearer;
	if (bearer_file.open(QIODevice::ReadOnly))
		bearer = QString::fromUtf8(bearer_file.readAll()).trimmed();
	const bool valid_bearer =
	    bearer.size() == 64 && std::all_of(bearer.cbegin(), bearer.cend(), [](QChar character) {
		    return (character >= QLatin1Char('0') && character <= QLatin1Char('9')) ||
		           (character >= QLatin1Char('a') && character <= QLatin1Char('f')) ||
		           (character >= QLatin1Char('A') && character <= QLatin1Char('F'));
	    });
	QSettings settings(QString::fromLatin1(settings_organization), QString::fromLatin1(settings_application));
	const bool configured = settings.value(QString::fromLatin1(configured_key), false).toBool();
	return std::make_unique<AgentClient>(QUrl(QStringLiteral("http://127.0.0.1:43127")),
	                                     valid_bearer ? bearer : QString{}, configured && valid_bearer, parent,
	                                     true);
}

bool AgentClient::is_ready() const
{
	return configuration_known_ && agent_available_ && !bearer_.isEmpty();
}

bool AgentClient::configuration_known() const
{
	return configuration_known_;
}

void AgentClient::probe(ResultHandler handler)
{
	send_request("GET", QStringLiteral("/health"), {}, false,
	             [this, handler = std::move(handler)](int status, QByteArray body) mutable {
		             const auto object = object_from(body);
		             agent_available_ =
		                 successful_status(status) &&
		                 object.value(QStringLiteral("status")).toString() == QStringLiteral("ok");
		             auto result = agent_available_ ? success_result(status) : parse_error(body, status);
		             if (!agent_available_ && result.message.isEmpty())
			             result.message = tr("O serviço local não respondeu.");
		             handler(std::move(result));
	             });
}

void AgentClient::save_config(const QString &bot_token, std::uint32_t api_id, const QString &api_hash, qint64 chat_id,
                              ResultHandler handler)
{
	forget_configuration_ready();
	QJsonObject payload{{QStringLiteral("bot_token"), bot_token},
	                    {QStringLiteral("api_id"), static_cast<qint64>(api_id)},
	                    {QStringLiteral("api_hash"), api_hash},
	                    {QStringLiteral("chat_id"), chat_id}};
	send_request("POST", QStringLiteral("/v1/config"), QJsonDocument(payload).toJson(QJsonDocument::Compact), true,
	             [this, handler = std::move(handler)](int status, QByteArray body) mutable {
		             agent_available_ = status > 0;
		             handler(parse_empty_success(body, status));
	             });
}

void AgentClient::detect_chat(const QString &bot_token, std::uint32_t api_id, const QString &api_hash,
                              const QString &challenge, ChatHandler handler)
{
	QJsonObject payload{{QStringLiteral("bot_token"), bot_token},
	                    {QStringLiteral("api_id"), static_cast<qint64>(api_id)},
	                    {QStringLiteral("api_hash"), api_hash},
	                    {QStringLiteral("challenge"), challenge}};
	send_request("POST", QStringLiteral("/v1/chat/detect"), QJsonDocument(payload).toJson(QJsonDocument::Compact),
	             true, [this, handler = std::move(handler)](int status, QByteArray body) mutable {
		             ChatDetectionResult detected;
		             if (!successful_status(status)) {
			             detected.result = parse_error(body, status);
			             handler(std::move(detected));
			             return;
		             }
		             const auto object = object_from(body);
		             detected.result = success_result(status);
		             detected.chat_id = object.value(QStringLiteral("chat_id")).toInteger();
		             detected.challenge = object.value(QStringLiteral("challenge")).toString();
		             detected.confirmed = object.value(QStringLiteral("confirmed")).toBool();
		             if (detected.chat_id == 0) {
			             detected.result = AgentResult{false, QStringLiteral("chat_not_found"),
			                                           tr("Nenhum chat foi encontrado. Envie o "
			                                              "comando mostrado e tente novamente."),
			                                           status};
		             }
		             handler(std::move(detected));
	             });
}

void AgentClient::test_send(ResultHandler handler)
{
	const QJsonObject payload{{QStringLiteral("message"), QStringLiteral("OBS Telegram Send está conectado.")}};
	send_request("POST", QStringLiteral("/v1/test-send"), QJsonDocument(payload).toJson(QJsonDocument::Compact),
	             true, [this, handler = std::move(handler)](int status, QByteArray body) mutable {
		             auto result = parse_empty_success(body, status);
		             if (result.ok) {
			             agent_available_ = true;
			             remember_configuration_ready();
		             }
		             handler(std::move(result));
	             });
}

void AgentClient::create_job(const QString &recording_path, const QString &display_name, CreatedJobHandler handler)
{
	const QJsonObject payload{{QStringLiteral("recording_path"), recording_path},
	                          {QStringLiteral("display_name"), display_name}};
	send_request("POST", QStringLiteral("/v1/jobs"), QJsonDocument(payload).toJson(QJsonDocument::Compact), true,
	             [this, handler = std::move(handler)](int status, QByteArray body) mutable {
		             CreatedJobResult created;
		             if (!successful_status(status)) {
			             created.result = parse_error(body, status);
			             handler(std::move(created));
			             return;
		             }
		             const auto object = object_from(body);
		             created.job_id = object.value(QStringLiteral("job_id")).toString();
		             created.state = object.value(QStringLiteral("state")).toString();
		             created.result = created.job_id.isEmpty()
		                                  ? AgentResult{false, QStringLiteral("invalid_response"),
		                                                tr("O serviço local retornou uma resposta "
		                                                   "inválida."),
		                                                status}
		                                  : success_result(status);
		             handler(std::move(created));
	             });
}

void AgentClient::get_job(const QString &job_id, JobStatusHandler handler)
{
	send_request("GET", QStringLiteral("/v1/jobs/%1").arg(QString::fromUtf8(QUrl::toPercentEncoding(job_id))), {},
	             true, [this, handler = std::move(handler)](int status, QByteArray body) mutable {
		             handler(parse_job_status(body, status));
	             });
}

void AgentClient::retry_job(const QString &job_id, JobStatusHandler handler)
{
	send_request(
	    "POST", QStringLiteral("/v1/jobs/%1/retry").arg(QString::fromUtf8(QUrl::toPercentEncoding(job_id))),
	    QByteArrayLiteral("{}"), true, [this, handler = std::move(handler)](int status, QByteArray body) mutable {
		    handler(parse_job_status(body, status));
	    });
}

AgentResult AgentClient::parse_error(const QByteArray &body, int http_status) const
{
	const auto object = object_from(body);
	AgentResult result;
	result.http_status = http_status;
	result.code = object.value(QStringLiteral("code")).toString();
	if (result.code == QStringLiteral("unauthorized"))
		result.message = tr("O agente local recusou a conexão. Reinicie o serviço e o OBS.");
	else if (result.code == QStringLiteral("invalid_configuration"))
		result.message = tr("Confira os dados do bot e do acesso ao Telegram.");
	else if (result.code == QStringLiteral("storage_unavailable"))
		result.message = tr("O armazenamento protegido não está disponível.");
	else if (result.code == QStringLiteral("local_server_unavailable"))
		result.message = tr("O serviço local do Telegram não conseguiu iniciar.");
	else if (result.code == QStringLiteral("telegram_not_configured"))
		result.message = tr("Conclua o teste final antes de enviar gravações.");
	else if (result.code == QStringLiteral("chat_not_found"))
		result.message = tr("Nenhum chat foi encontrado. Envie o comando mostrado e tente novamente.");
	else if (result.code == QStringLiteral("telegram_request_failed"))
		result.message = tr("O Telegram não conseguiu concluir esta ação. Confira sua conexão.");
	else if (result.code == QStringLiteral("invalid_job"))
		result.message = http_status == 413 ? tr("A gravação ultrapassa o limite de 2 GB.")
		                                    : tr("A gravação não pôde ser preparada para envio.");
	else if (result.code == QStringLiteral("job_not_found"))
		result.message = tr("Este envio não foi encontrado pelo serviço local.");
	else if (result.code == QStringLiteral("job_not_retryable"))
		result.message = tr("Este envio não pode ser tentado novamente com segurança.");
	else
		result.message = safe_message(object.value(QStringLiteral("message")).toString());
	if (result.message.isEmpty()) {
		result.message = http_status == 0 ? tr("O serviço local não respondeu.")
		                                  : tr("O serviço local não conseguiu concluir esta ação.");
	}
	return result;
}

JobStatusResult AgentClient::parse_job_status(const QByteArray &body, int http_status) const
{
	JobStatusResult status;
	if (!successful_status(http_status)) {
		status.result = parse_error(body, http_status);
		return status;
	}
	const auto object = object_from(body);
	status.job_id = object.value(QStringLiteral("job_id")).toString();
	status.filename = QFileInfo(object.value(QStringLiteral("filename")).toString()).fileName();
	status.state = object.value(QStringLiteral("state")).toString();
	if (object.value(QStringLiteral("progress_percent")).isDouble()) {
		const int percent = object.value(QStringLiteral("progress_percent")).toInt(-1);
		if (percent >= 0 && percent <= 100)
			status.progress_percent = percent;
	}
	status.message = safe_message(object.value(QStringLiteral("message")).toString());
	if (status.message.isEmpty())
		status.message = safe_message(object.value(QStringLiteral("retryable_error")).toString());
	status.retryable = status.state == QStringLiteral("failed") &&
	                   !object.value(QStringLiteral("retryable_error")).toString().isEmpty();
	status.result = status.job_id.isEmpty() || status.filename.isEmpty() || status.state.isEmpty()
	                    ? AgentResult{false, QStringLiteral("invalid_response"),
	                                  tr("O serviço local retornou uma resposta inválida."), http_status}
	                    : success_result(http_status);
	return status;
}

void AgentClient::send_request(const QByteArray &method, const QString &path, const QByteArray &body,
                               bool authenticated, ReplyHandler handler)
{
	if (authenticated && bearer_.isEmpty()) {
		handler(0, {});
		return;
	}
	QUrl url = base_url_;
	url.setPath(path);
	QNetworkRequest request(url);
	request.setHeader(QNetworkRequest::ContentTypeHeader, QStringLiteral("application/json"));
	request.setAttribute(QNetworkRequest::RedirectPolicyAttribute, QNetworkRequest::ManualRedirectPolicy);
	request.setTransferTimeout(15000);
	if (authenticated)
		request.setRawHeader("Authorization", QByteArrayLiteral("Bearer ") + bearer_.toUtf8());
	QNetworkReply *reply = method == QByteArrayLiteral("GET") ? network_->get(request)
	                                                          : network_->sendCustomRequest(request, method, body);
	connect(reply, &QNetworkReply::finished, this, [reply, handler = std::move(handler)]() mutable {
		const int status = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
		const auto body = reply->readAll();
		reply->deleteLater();
		handler(status, body);
	});
}

AgentResult AgentClient::parse_empty_success(const QByteArray &body, int http_status) const
{
	return successful_status(http_status) ? success_result(http_status) : parse_error(body, http_status);
}

QString AgentClient::safe_message(QString message) const
{
	if (!bearer_.isEmpty())
		message.replace(bearer_, QStringLiteral("[dado protegido]"), Qt::CaseSensitive);
	return message.left(500);
}

void AgentClient::remember_configuration_ready()
{
	configuration_known_ = true;
	if (!persist_configuration_state_)
		return;
	QSettings settings(QString::fromLatin1(settings_organization), QString::fromLatin1(settings_application));
	settings.setValue(QString::fromLatin1(configured_key), true);
}

void AgentClient::forget_configuration_ready()
{
	configuration_known_ = false;
	if (!persist_configuration_state_)
		return;
	QSettings settings(QString::fromLatin1(settings_organization), QString::fromLatin1(settings_application));
	settings.setValue(QString::fromLatin1(configured_key), false);
}
