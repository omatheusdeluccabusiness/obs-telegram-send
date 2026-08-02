#include "recording_controller.hpp"
#include "send_confirmation_dialog.hpp"

#include <obs-frontend-api.h>
#include <obs-module.h>

#include <QFileInfo>
#include <QWidget>

#include <cmath>
#include <memory>
#include <optional>
#include <string>

OBS_DECLARE_MODULE()
OBS_MODULE_USE_DEFAULT_LOCALE("obs-telegram-send", "en-US")

namespace {

std::unique_ptr<SendConfirmationDialog> confirmation_dialog;
std::unique_ptr<RecordingController> recording_controller;

std::uint64_t completed_recording_duration_ms()
{
	auto *output = obs_frontend_get_recording_output();
	if (!output)
		return 0;

	auto *video = obs_output_video(output);
	const double frames_per_second = video ? video_output_get_frame_rate(video) : 0.0;
	const int frames = obs_output_get_total_frames(output);
	if (frames_per_second <= 0.0 || frames <= 0)
		return 0;

	return static_cast<std::uint64_t>(std::llround(frames * 1000.0 / frames_per_second));
}

std::optional<RecordingMetadata> resolve_completed_recording()
{
	char *obs_path = obs_frontend_get_last_recording();
	if (!obs_path || !*obs_path) {
		bfree(obs_path);
		blog(LOG_WARNING, "Recording stopped without a completed output path");
		return std::nullopt;
	}

	const QString path = QString::fromUtf8(obs_path);
	bfree(obs_path);
	const QFileInfo file(path);
	if (!file.exists() || !file.isFile()) {
		blog(LOG_WARNING, "Completed recording path is not a readable file");
		return std::nullopt;
	}

	return RecordingMetadata{file.absoluteFilePath().toStdString(), file.fileName().toStdString(),
				 completed_recording_duration_ms(), static_cast<std::uint64_t>(file.size())};
}

void frontend_event(obs_frontend_event event, void *private_data)
{
	static_cast<RecordingController *>(private_data)->on_frontend_event(event);
}

} // namespace

bool obs_module_load(void)
{
	auto *main_window = static_cast<QWidget *>(obs_frontend_get_main_window());
	confirmation_dialog = std::make_unique<SendConfirmationDialog>(main_window);
	recording_controller = std::make_unique<RecordingController>(
		[](const std::string &, const std::string &) {
			// Task 6 replaces this adapter with AgentClient::create_job.
			blog(LOG_WARNING, "Telegram agent client is not connected");
		},
		resolve_completed_recording,
		[](const RecordingMetadata &metadata) { confirmation_dialog->show_for(metadata); });
	confirmation_dialog->set_send_confirmed_handler(
		[](const RecordingMetadata &) { recording_controller->confirm_send(); });
	obs_frontend_add_event_callback(frontend_event, recording_controller.get());
	return true;
}

void obs_module_unload(void)
{
	if (recording_controller)
		obs_frontend_remove_event_callback(frontend_event, recording_controller.get());
	recording_controller.reset();
	confirmation_dialog.reset();
}
