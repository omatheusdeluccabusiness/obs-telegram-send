#include "agent_client.hpp"
#include "onboarding_dialog.hpp"

#include <gtest/gtest.h>

#include <QAbstractButton>
#include <QJsonObject>
#include <QWizardPage>

#include <optional>

namespace {

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
