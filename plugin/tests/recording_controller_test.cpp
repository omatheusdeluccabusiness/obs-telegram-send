#include "recording_controller.hpp"
#include "send_confirmation_dialog.hpp"

#include <gtest/gtest.h>

#include <QApplication>
#include <QCheckBox>
#include <QPushButton>

#include <cstdint>
#include <string>
#include <utility>
#include <vector>

namespace {

struct CreatedJob {
	std::string path;
	std::string display_name;
};

class FakeAgentClient {
public:
	void create_job(const std::string &path, const std::string &display_name)
	{
		created_jobs_.push_back({path, display_name});
	}

	const std::vector<CreatedJob> &created_jobs() const { return created_jobs_; }

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

TEST(SendConfirmationDialog, ConfirmsOnlyAfterCheckedSendButtonClick)
{
	SendConfirmationDialog dialog(nullptr);
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

} // namespace

int main(int argc, char **argv)
{
	QApplication app(argc, argv);
	::testing::InitGoogleTest(&argc, argv);
	return RUN_ALL_TESTS();
}
