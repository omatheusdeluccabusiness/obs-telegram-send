#include "recording_controller.hpp"
#include "send_confirmation_dialog.hpp"

#include <gtest/gtest.h>

#include <QApplication>
#include <QCheckBox>
#include <QLabel>
#include <QProgressBar>
#include <QPushButton>
#include <QTimer>

#include <cstdint>
#include <string>
#include <utility>
#include <vector>

namespace {

struct CreatedJob
{
	std::string path;
	std::string display_name;
};

class FakeAgentClient
{
      public:
	void create_job(const std::string &path, const std::string &display_name)
	{
		created_jobs_.push_back({path, display_name});
	}

	const std::vector<CreatedJob> &created_jobs() const
	{
		return created_jobs_;
	}

      private:
	std::vector<CreatedJob> created_jobs_;
};

RecordingMetadata metadata(std::string filename, std::uint64_t size)
{
	return RecordingMetadata{"/recordings/" + filename, std::move(filename), 0, size};
}

SendConfirmationDialog::ConfirmationToken begin_send(SendConfirmationDialog &dialog, RecordingMetadata recording)
{
	dialog.set_send_confirmed_handler([](const RecordingMetadata &, std::uint64_t) {});
	const auto token = dialog.show_for(std::move(recording));
	auto *consent = dialog.findChild<QCheckBox *>(QStringLiteral("sendConsentCheckBox"));
	auto *send = dialog.findChild<QPushButton *>(QStringLiteral("sendNowButton"));
	if (consent && send) {
		consent->setChecked(true);
		send->click();
	}
	return token;
}

TEST(RecordingController, DoesNotCreateUploadBeforeCheckboxConfirmation)
{
	FakeAgentClient agent;
	RecordingController controller(agent);

	controller.recording_finished(metadata("take.mp4", 123));

	EXPECT_TRUE(agent.created_jobs().empty());
}

TEST(RecordingController, CreatesUploadOnlyAfterExplicitConfirmation)
{
	FakeAgentClient agent;
	RecordingController controller(agent);
	controller.recording_finished(metadata("confirmed-take.mkv", 456));

	controller.confirm_send();

	ASSERT_EQ(agent.created_jobs().size(), 1U);
	EXPECT_EQ(agent.created_jobs().front().path, "/recordings/confirmed-take.mkv");
	EXPECT_EQ(agent.created_jobs().front().display_name, "confirmed-take.mkv");
}

TEST(RecordingController, OpensConfirmationOnlyForRecordingStopped)
{
	FakeAgentClient agent;
	int resolve_calls = 0;
	std::vector<RecordingMetadata> shown_recordings;
	RecordingController controller(
	    [&agent](const std::string &path, const std::string &display_name) {
		    agent.create_job(path, display_name);
	    },
	    [&resolve_calls] {
		    ++resolve_calls;
		    return std::optional<RecordingMetadata>(metadata("stopped.mp4", 789));
	    },
	    [&shown_recordings](const RecordingMetadata &recording) { shown_recordings.push_back(recording); });

	controller.on_frontend_event(OBS_FRONTEND_EVENT_RECORDING_STARTED);
	EXPECT_EQ(resolve_calls, 0);
	EXPECT_TRUE(shown_recordings.empty());

	controller.on_frontend_event(OBS_FRONTEND_EVENT_RECORDING_STOPPED);
	ASSERT_EQ(resolve_calls, 1);
	ASSERT_EQ(shown_recordings.size(), 1U);
	EXPECT_EQ(shown_recordings.front().path, "/recordings/stopped.mp4");
	EXPECT_TRUE(agent.created_jobs().empty());
}

TEST(SendConfirmationDialog, ResetsUncheckedConsentForEveryRecording)
{
	SendConfirmationDialog dialog(nullptr);
	dialog.set_agent_ready(true);
	dialog.show_for(metadata("first.mp4", 100));
	auto *consent = dialog.findChild<QCheckBox *>("sendConsentCheckBox");
	auto *send = dialog.findChild<QPushButton *>("sendNowButton");
	ASSERT_NE(consent, nullptr);
	ASSERT_NE(send, nullptr);
	EXPECT_FALSE(consent->isChecked());
	EXPECT_FALSE(send->isEnabled());

	consent->setChecked(true);
	EXPECT_TRUE(send->isEnabled());
	dialog.show_for(metadata("second.mkv", 200));

	EXPECT_FALSE(consent->isChecked());
	EXPECT_FALSE(send->isEnabled());
}

TEST(SendConfirmationDialog, CheckedConsentNeverEnablesSendWhileAgentIsNotReady)
{
	SendConfirmationDialog dialog(nullptr);
	dialog.show_for(metadata("not-ready.mp4", 100));
	auto *consent = dialog.findChild<QCheckBox *>("sendConsentCheckBox");
	auto *send = dialog.findChild<QPushButton *>("sendNowButton");
	ASSERT_NE(consent, nullptr);
	ASSERT_NE(send, nullptr);

	consent->setChecked(true);

	EXPECT_FALSE(send->isEnabled());
}

TEST(SendConfirmationDialog, ConfirmsOnlyAfterCheckedSendButtonClick)
{
	SendConfirmationDialog dialog(nullptr);
	dialog.set_agent_ready(true);
	int confirmations = 0;
	dialog.set_send_confirmed_handler(
	    [&confirmations](const RecordingMetadata &, std::uint64_t) { ++confirmations; });
	dialog.show_for(metadata("take.mp4", 300));
	auto *consent = dialog.findChild<QCheckBox *>("sendConsentCheckBox");
	auto *send = dialog.findChild<QPushButton *>("sendNowButton");
	ASSERT_NE(consent, nullptr);
	ASSERT_NE(send, nullptr);
	EXPECT_EQ(confirmations, 0);

	consent->setChecked(true);
	EXPECT_EQ(confirmations, 0);
	send->click();

	EXPECT_EQ(confirmations, 1);
}

TEST(SendConfirmationDialog, StaysOpenToShowRealJobStateAfterConfirmation)
{
	SendConfirmationDialog dialog(nullptr);
	dialog.set_agent_ready(true);
	bool visible_during_handler = false;
	dialog.set_send_confirmed_handler([&dialog, &visible_during_handler](const RecordingMetadata &, std::uint64_t) {
		visible_during_handler = dialog.isVisible();
	});
	dialog.show_for(metadata("take.mp4", 300));
	auto *consent = dialog.findChild<QCheckBox *>("sendConsentCheckBox");
	auto *send = dialog.findChild<QPushButton *>("sendNowButton");
	ASSERT_NE(consent, nullptr);
	ASSERT_NE(send, nullptr);
	ASSERT_TRUE(dialog.isVisible());

	consent->setChecked(true);
	send->click();

	EXPECT_TRUE(visible_during_handler);
	EXPECT_TRUE(dialog.isVisible());
	EXPECT_NE(dialog.result(), QDialog::Accepted);
}

TEST(SendConfirmationDialog, PollsUploadingJobEvery500MillisecondsAndShowsSafeFields)
{
	SendConfirmationDialog dialog(nullptr);
	dialog.set_agent_ready(true);
	const auto token = begin_send(dialog, metadata("take.mp4", 300));
	dialog.show_job_created(
	    token,
	    CreatedJobResult{AgentResult{true, {}, {}, 201}, QStringLiteral("job-1"), QStringLiteral("queued")});
	JobStatusResult status;
	status.result = AgentResult{true, {}, {}, 200};
	status.job_id = QStringLiteral("job-1");
	status.filename = QStringLiteral("take.mp4");
	status.state = QStringLiteral("uploading");
	status.progress_percent = 37;
	status.message = QStringLiteral("Enviando com segurança");
	dialog.apply_job_status(status);

	auto *timer = dialog.findChild<QTimer *>(QStringLiteral("jobPollTimer"));
	auto *filename = dialog.findChild<QLabel *>(QStringLiteral("jobFilenameLabel"));
	auto *progress = dialog.findChild<QProgressBar *>(QStringLiteral("jobProgressBar"));
	auto *message = dialog.findChild<QLabel *>(QStringLiteral("jobMessageLabel"));
	ASSERT_NE(timer, nullptr);
	ASSERT_NE(filename, nullptr);
	ASSERT_NE(progress, nullptr);
	ASSERT_NE(message, nullptr);
	EXPECT_EQ(timer->interval(), 500);
	EXPECT_TRUE(timer->isActive());
	EXPECT_EQ(filename->text(), QStringLiteral("take.mp4"));
	EXPECT_EQ(filename->textFormat(), Qt::PlainText);
	EXPECT_EQ(progress->value(), 37);
	EXPECT_EQ(message->text(), QStringLiteral("Enviando com segurança"));
	EXPECT_EQ(message->textFormat(), Qt::PlainText);
}

TEST(SendConfirmationDialog, QueuesNewRecordingUntilActiveJobIsTerminal)
{
	SendConfirmationDialog dialog(nullptr);
	dialog.set_agent_ready(true);
	const auto first_token = begin_send(dialog, metadata("first.mp4", 300));
	dialog.show_job_created(
	    first_token,
	    CreatedJobResult{AgentResult{true, {}, {}, 201}, QStringLiteral("job-first"), QStringLiteral("queued")});

	JobStatusResult uploading;
	uploading.result = AgentResult{true, {}, {}, 200};
	uploading.job_id = QStringLiteral("job-first");
	uploading.filename = QStringLiteral("first.mp4");
	uploading.state = QStringLiteral("uploading");
	dialog.apply_job_status(uploading);

	const auto second_token = dialog.show_for(metadata("second.mkv", 400));
	auto *name = dialog.findChild<QLabel *>(QStringLiteral("recordingNameLabel"));
	auto *timer = dialog.findChild<QTimer *>(QStringLiteral("jobPollTimer"));
	ASSERT_NE(name, nullptr);
	ASSERT_NE(timer, nullptr);
	EXPECT_NE(second_token, first_token);
	EXPECT_EQ(name->text(), QStringLiteral("first.mp4"));
	EXPECT_TRUE(timer->isActive());

	JobStatusResult completed = uploading;
	completed.state = QStringLiteral("completed");
	completed.progress_percent = 100;
	dialog.apply_job_status(completed);

	EXPECT_EQ(name->text(), QStringLiteral("second.mkv"));
	EXPECT_FALSE(timer->isActive());
	EXPECT_FALSE(dialog.findChild<QCheckBox *>(QStringLiteral("sendConsentCheckBox"))->isChecked());
}

TEST(SendConfirmationDialog, IgnoresLateCreateResponseFromEarlierConfirmation)
{
	SendConfirmationDialog dialog(nullptr);
	dialog.set_agent_ready(true);
	std::uint64_t submitted_token = 0;
	dialog.set_send_confirmed_handler(
	    [&submitted_token](const RecordingMetadata &, std::uint64_t token) { submitted_token = token; });
	const auto first_token = dialog.show_for(metadata("first.mp4", 300));
	auto *consent = dialog.findChild<QCheckBox *>(QStringLiteral("sendConsentCheckBox"));
	auto *send = dialog.findChild<QPushButton *>(QStringLiteral("sendNowButton"));
	ASSERT_NE(consent, nullptr);
	ASSERT_NE(send, nullptr);
	consent->setChecked(true);
	send->click();
	ASSERT_EQ(submitted_token, first_token);

	const auto second_token = dialog.show_for(metadata("second.mkv", 400));
	ASSERT_NE(second_token, first_token);
	auto *name = dialog.findChild<QLabel *>(QStringLiteral("recordingNameLabel"));
	ASSERT_NE(name, nullptr);
	EXPECT_EQ(name->text(), QStringLiteral("first.mp4"));

	dialog.show_job_created(
	    first_token,
	    CreatedJobResult{AgentResult{false, QStringLiteral("invalid_job"), QStringLiteral("falhou"), 400}, {}, {}});
	EXPECT_EQ(name->text(), QStringLiteral("second.mkv"));

	dialog.show_job_created(
	    first_token,
	    CreatedJobResult{AgentResult{true, {}, {}, 201}, QStringLiteral("job-first"), QStringLiteral("queued")});

	auto *job_filename = dialog.findChild<QLabel *>(QStringLiteral("jobFilenameLabel"));
	auto *timer = dialog.findChild<QTimer *>(QStringLiteral("jobPollTimer"));
	ASSERT_NE(name, nullptr);
	ASSERT_NE(job_filename, nullptr);
	ASSERT_NE(timer, nullptr);
	EXPECT_EQ(name->text(), QStringLiteral("second.mkv"));
	EXPECT_FALSE(job_filename->isVisible());
	EXPECT_FALSE(timer->isActive());
}

TEST(SendConfirmationDialog, RetriesTransientPollingFailuresWithBoundedBackoff)
{
	SendConfirmationDialog dialog(nullptr);
	dialog.set_agent_ready(true);
	const auto token = begin_send(dialog, metadata("take.mp4", 300));
	dialog.show_job_created(
	    token,
	    CreatedJobResult{AgentResult{true, {}, {}, 201}, QStringLiteral("job-1"), QStringLiteral("queued")});
	auto *timer = dialog.findChild<QTimer *>(QStringLiteral("jobPollTimer"));
	auto *message = dialog.findChild<QLabel *>(QStringLiteral("jobMessageLabel"));
	ASSERT_NE(timer, nullptr);
	ASSERT_NE(message, nullptr);

	JobStatusResult transient;
	transient.result = AgentResult{false, {}, QStringLiteral("O serviço local não respondeu."), 0};
	dialog.apply_job_status(transient);
	EXPECT_TRUE(timer->isActive());
	EXPECT_EQ(timer->interval(), 1000);
	EXPECT_TRUE(message->text().contains(QStringLiteral("Tentando novamente")));

	dialog.apply_job_status(transient);
	EXPECT_TRUE(timer->isActive());
	EXPECT_EQ(timer->interval(), 2000);
	for (int attempt = 0; attempt < 8; ++attempt)
		dialog.apply_job_status(transient);
	EXPECT_EQ(timer->interval(), 5000);

	JobStatusResult uploading;
	uploading.result = AgentResult{true, {}, {}, 200};
	uploading.job_id = QStringLiteral("job-1");
	uploading.filename = QStringLiteral("take.mp4");
	uploading.state = QStringLiteral("uploading");
	dialog.apply_job_status(uploading);
	EXPECT_TRUE(timer->isActive());
	EXPECT_EQ(timer->interval(), 500);

	uploading.state = QStringLiteral("completed");
	dialog.apply_job_status(uploading);
	EXPECT_FALSE(timer->isActive());

	const auto second_token = begin_send(dialog, metadata("take-2.mp4", 400));
	dialog.show_job_created(
	    second_token,
	    CreatedJobResult{AgentResult{true, {}, {}, 201}, QStringLiteral("job-2"), QStringLiteral("queued")});
	JobStatusResult invalid_protocol;
	invalid_protocol.result = AgentResult{false, QStringLiteral("invalid_response"),
	                                      QStringLiteral("Resposta inválida"), 200};
	dialog.apply_job_status(invalid_protocol);
	EXPECT_FALSE(timer->isActive());
	EXPECT_EQ(message->text(), QStringLiteral("Resposta inválida"));
}

} // namespace

int main(int argc, char **argv)
{
	QApplication app(argc, argv);
	::testing::InitGoogleTest(&argc, argv);
	return RUN_ALL_TESTS();
}
