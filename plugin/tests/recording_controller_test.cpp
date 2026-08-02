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
	dialog.set_send_confirmed_handler([&confirmations](const RecordingMetadata &) { ++confirmations; });
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
	dialog.set_send_confirmed_handler([&dialog, &visible_during_handler](const RecordingMetadata &) {
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
	dialog.show_for(metadata("take.mp4", 300));
	dialog.show_job_created(
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

} // namespace

int main(int argc, char **argv)
{
	QApplication app(argc, argv);
	::testing::InitGoogleTest(&argc, argv);
	return RUN_ALL_TESTS();
}
