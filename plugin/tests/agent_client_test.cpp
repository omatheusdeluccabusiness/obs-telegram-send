#include "agent_client.hpp"
#include "onboarding_dialog.hpp"

#include <gtest/gtest.h>

#include <QAbstractButton>
#include <QCoreApplication>
#include <QElapsedTimer>
#include <QEventLoop>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTcpServer>
#include <QTcpSocket>
#include <QThread>
#include <QWizardPage>

#include <deque>
#include <functional>
#include <memory>
#include <optional>

namespace {

struct HttpResponse
{
	int status;
	QByteArray body;
};

class LoopbackHttpServer : public QObject
{
      public:
	LoopbackHttpServer()
	{
		EXPECT_TRUE(server_.listen(QHostAddress::LocalHost, 0));
		connect(&server_, &QTcpServer::newConnection, this, [this] {
			while (auto *socket = server_.nextPendingConnection()) {
				auto bytes = std::make_shared<QByteArray>();
				connect(socket, &QTcpSocket::readyRead, this, [this, socket, bytes] {
					bytes->append(socket->readAll());
					const auto header_end = bytes->indexOf("\r\n\r\n");
					if (header_end < 0)
						return;
					int content_length = 0;
					for (const auto &line : bytes->left(header_end).split('\n')) {
						const auto trimmed = line.trimmed();
						if (trimmed.toLower().startsWith("content-length:"))
							content_length = trimmed.mid(QByteArrayLiteral("content-length:").size()).trimmed().toInt();
					}
					if (bytes->size() < header_end + 4 + content_length)
						return;
					requests_.push_back(bytes->left(header_end + 4 + content_length));
					ASSERT_FALSE(responses_.empty());
					const auto response = responses_.front();
					responses_.pop_front();
					const auto reason = response.status >= 500 ? QByteArrayLiteral("Service Unavailable")
					                                             : QByteArrayLiteral("OK");
					QByteArray wire = QByteArrayLiteral("HTTP/1.1 ") + QByteArray::number(response.status) + ' ' + reason +
					                  QByteArrayLiteral("\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: ") +
					                  QByteArray::number(response.body.size()) + QByteArrayLiteral("\r\n\r\n") + response.body;
					socket->write(wire);
					socket->disconnectFromHost();
				});
			}
		});
	}

	QUrl base_url() const
	{
		return QUrl(QStringLiteral("http://127.0.0.1:%1").arg(server_.serverPort()));
	}

	void respond(int status, QByteArray body)
	{
		responses_.push_back(HttpResponse{status, std::move(body)});
	}

	const std::vector<QByteArray> &requests() const
	{
		return requests_;
	}

      private:
	QTcpServer server_;
	std::deque<HttpResponse> responses_;
	std::vector<QByteArray> requests_;
};

bool wait_until(const std::function<bool()> &condition, int timeout_ms = 2000)
{
	QElapsedTimer elapsed;
	elapsed.start();
	while (!condition() && elapsed.elapsed() < timeout_ms) {
		QCoreApplication::processEvents(QEventLoop::AllEvents, 10);
		QThread::msleep(1);
	}
	return condition();
}

TEST(AgentClient, NeverIncludesBearerInVisibleError)
{
	AgentClient client(QUrl(QStringLiteral("http://127.0.0.1:43127")), QStringLiteral("install-secret"), false);

	const auto result =
	    client.parse_error(QByteArrayLiteral("{\"code\":\"invalid\",\"message\":\"invalid install-secret\"}"), 400);

	EXPECT_EQ(result.code, QStringLiteral("invalid"));
	EXPECT_EQ(result.message, QStringLiteral("invalid [dado protegido]"));
	EXPECT_FALSE(result.message.contains(QStringLiteral("install-secret")));
}

TEST(AgentClient, KnownAgentErrorsArePresentedInPortuguese)
{
	AgentClient client(QUrl(QStringLiteral("http://127.0.0.1:43127")), QStringLiteral("install-secret"), false);

	const auto result = client.parse_error(QByteArrayLiteral("{\"code\":\"unauthorized\",\"message\":\"A valid "
	                                                         "install bearer is required.\"}"),
	                                       401);

	EXPECT_EQ(result.message, QStringLiteral("O agente local recusou a conexão. Reinicie o serviço e o OBS."));
}

TEST(AgentClient, JobStatusExposesOnlySafeProtocolFields)
{
	AgentClient client(QUrl(QStringLiteral("http://127.0.0.1:43127")), QStringLiteral("install-secret"), true);

	const auto status = client.parse_job_status(
	    QByteArrayLiteral("{\"job_id\":\"job-1\",\"filename\":\"take.mp4\",\"state\":\"uploading\","
	                      "\"progress_percent\":37,\"message\":\"Enviando\","
	                      "\"recording_path\":\"/Users/private/"
	                      "take.mp4\",\"bot_token\":\"secret\"}"));

	ASSERT_TRUE(status.result.ok);
	EXPECT_EQ(status.job_id, QStringLiteral("job-1"));
	EXPECT_EQ(status.filename, QStringLiteral("take.mp4"));
	EXPECT_EQ(status.state, QStringLiteral("uploading"));
	ASSERT_TRUE(status.progress_percent.has_value());
	EXPECT_EQ(*status.progress_percent, 37);
	EXPECT_EQ(status.message, QStringLiteral("Enviando"));
}

TEST(AgentClient, ReadyRequiresKnownConfigurationAndLiveAgentProbe)
{
	AgentClient client(QUrl(QStringLiteral("http://127.0.0.1:1")), QStringLiteral("install-secret"), false);

	EXPECT_FALSE(client.is_ready());
	EXPECT_FALSE(client.configuration_known());
}

TEST(AgentClient, ChangingConfigurationRevokesEarlierConfirmationUntilARealTestSucceeds)
{
	AgentClient client(QUrl(QStringLiteral("http://127.0.0.1:1")), QStringLiteral("install-secret"), true);
	ASSERT_TRUE(client.configuration_known());

	client.save_config(QStringLiteral("123:token"), 123, QStringLiteral("0123456789abcdef0123456789abcdef"), 42,
	                   [](AgentResult) {});

	EXPECT_FALSE(client.configuration_known());
}

TEST(AgentClient, RealLoopbackTransportPreservesRouteAuthBodyAndDecodesTransientRecovery)
{
	LoopbackHttpServer server;
	server.respond(201, QByteArrayLiteral("{\"job_id\":\"job-7\",\"state\":\"queued\"}"));
	server.respond(503, QByteArrayLiteral("{\"code\":\"storage_unavailable\"}"));
	server.respond(200, QByteArrayLiteral("{\"job_id\":\"job-7\",\"filename\":\"take.mp4\","
	                                     "\"state\":\"uploading\",\"progress_percent\":41}"));
	AgentClient client(server.base_url(), QStringLiteral("loopback-secret"), true);

	std::optional<CreatedJobResult> created;
	client.create_job(QStringLiteral("/recordings/take.mp4"), QStringLiteral("take.mp4"),
	                  [&created](CreatedJobResult result) { created = std::move(result); });
	ASSERT_TRUE(wait_until([&created] { return created.has_value(); }));
	ASSERT_TRUE(created->result.ok);
	EXPECT_EQ(created->job_id, QStringLiteral("job-7"));

	std::optional<JobStatusResult> unavailable;
	client.get_job(QStringLiteral("job-7"),
	               [&unavailable](JobStatusResult result) { unavailable = std::move(result); });
	ASSERT_TRUE(wait_until([&unavailable] { return unavailable.has_value(); }));
	EXPECT_FALSE(unavailable->result.ok);
	EXPECT_EQ(unavailable->result.http_status, 503);
	EXPECT_EQ(unavailable->result.code, QStringLiteral("storage_unavailable"));

	std::optional<JobStatusResult> recovered;
	client.get_job(QStringLiteral("job-7"), [&recovered](JobStatusResult result) { recovered = std::move(result); });
	ASSERT_TRUE(wait_until([&recovered] { return recovered.has_value(); }));
	ASSERT_TRUE(recovered->result.ok);
	EXPECT_EQ(recovered->state, QStringLiteral("uploading"));
	EXPECT_EQ(recovered->progress_percent, 41);

	ASSERT_EQ(server.requests().size(), 3U);
	EXPECT_TRUE(server.requests()[0].startsWith("POST /v1/jobs HTTP/1.1\r\n"));
	EXPECT_TRUE(server.requests()[0].toLower().contains("authorization: bearer loopback-secret\r\n"));
	const auto first_body = server.requests()[0].mid(server.requests()[0].indexOf("\r\n\r\n") + 4);
	const auto payload = QJsonDocument::fromJson(first_body).object();
	EXPECT_EQ(payload.value(QStringLiteral("recording_path")).toString(), QStringLiteral("/recordings/take.mp4"));
	EXPECT_EQ(payload.value(QStringLiteral("display_name")).toString(), QStringLiteral("take.mp4"));
	EXPECT_TRUE(server.requests()[1].startsWith("GET /v1/jobs/job-7 HTTP/1.1\r\n"));
	EXPECT_TRUE(server.requests()[2].startsWith("GET /v1/jobs/job-7 HTTP/1.1\r\n"));
}

TEST(OnboardingDialog, UsesTheFourRequiredPortuguesePages)
{
	AgentClient client(QUrl(QStringLiteral("http://127.0.0.1:1")), QStringLiteral("install-secret"), false);
	OnboardingDialog dialog(client, nullptr);
	QStringList titles;
	for (int id : dialog.pageIds())
		titles.push_back(dialog.page(id)->title());

	EXPECT_EQ(titles, (QStringList{QStringLiteral("Seu bot"), QStringLiteral("Seu acesso ao Telegram"),
	                               QStringLiteral("Seu chat"), QStringLiteral("Teste final")}));
}

TEST(OnboardingDialog, ProvidesDirectOfficialLinksAndExplicitActions)
{
	AgentClient client(QUrl(QStringLiteral("http://127.0.0.1:1")), QStringLiteral("install-secret"), false);
	OnboardingDialog dialog(client, nullptr);

	auto *botfather = dialog.findChild<QAbstractButton *>(QStringLiteral("openBotFatherButton"));
	auto *telegram_app = dialog.findChild<QAbstractButton *>(QStringLiteral("openTelegramAppButton"));
	auto *detect = dialog.findChild<QAbstractButton *>(QStringLiteral("detectChatButton"));
	auto *test_send = dialog.findChild<QAbstractButton *>(QStringLiteral("testSendButton"));
	ASSERT_NE(botfather, nullptr);
	ASSERT_NE(telegram_app, nullptr);
	ASSERT_NE(detect, nullptr);
	ASSERT_NE(test_send, nullptr);
	EXPECT_EQ(botfather->property("targetUrl").toUrl(), QUrl(QStringLiteral("https://t.me/BotFather")));
	EXPECT_EQ(telegram_app->property("targetUrl").toUrl(), QUrl(QStringLiteral("https://my.telegram.org")));
	EXPECT_EQ(detect->text(), QStringLiteral("Detectar meu chat"));
	EXPECT_EQ(test_send->text(), QStringLiteral("Enviar mensagem de teste"));
}

} // namespace
