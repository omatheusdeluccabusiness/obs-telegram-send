#include "agent_client.hpp"
#include "onboarding_dialog.hpp"
#include "recording_controller.hpp"
#include "send_confirmation_dialog.hpp"

#include <obs-frontend-api.h>
#include <obs-module.h>

#include <QAction>
#include <QFileInfo>
#include <QPointer>
#include <QWidget>

#include <cmath>
#include <memory>
#include <optional>
#include <string>

OBS_DECLARE_MODULE()
OBS_MODULE_USE_DEFAULT_LOCALE("obs-telegram-send", "en-US")

namespace {

QPointer<AgentClient> agent_client;
QPointer<OnboardingDialog> onboarding_dialog;
QPointer<SendConfirmationDialog> confirmation_dialog;
std::unique_ptr<RecordingController> recording_controller;
QPointer<QAction> tools_action;

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
	auto client = AgentClient::from_system(main_window);
	agent_client = client.release();
	onboarding_dialog = new OnboardingDialog(*agent_client, main_window);
	confirmation_dialog = new SendConfirmationDialog(agent_client, main_window);
	recording_controller = std::make_unique<RecordingController>(
	    [](const std::string &path, const std::string &display_name) {
		    agent_client->create_job(
		        QString::fromStdString(path), QString::fromStdString(display_name),
		        [](CreatedJobResult) {});
	    },
	    resolve_completed_recording,
	    [](const RecordingMetadata &metadata) { confirmation_dialog->show_for(metadata); });
	confirmation_dialog->set_send_confirmed_handler(
	    [](const RecordingMetadata &metadata, SendConfirmationDialog::ConfirmationToken token) {
		    agent_client->create_job(
		        QString::fromStdString(metadata.path), QString::fromStdString(metadata.display_name),
		        [token](CreatedJobResult result) {
			        if (result.result.ok && result.job_id.isEmpty()) {
				        result.result =
				            AgentResult{false, QStringLiteral("invalid_response"),
				                        QObject::tr("O serviço local retornou uma resposta inválida."),
				                        result.result.http_status};
			        }
			        if (confirmation_dialog)
				        confirmation_dialog->show_job_created(token, std::move(result));
		        });
	    });
	obs_frontend_add_event_callback(frontend_event, recording_controller.get());
	tools_action = static_cast<QAction *>(obs_frontend_add_tools_menu_qaction("Telegram Send"));
	QObject::connect(tools_action, &QAction::triggered, onboarding_dialog.get(), [] {
		onboarding_dialog->start_over();
		onboarding_dialog->show();
		onboarding_dialog->raise();
		onboarding_dialog->activateWindow();
	});
	return true;
}

void obs_module_unload(void)
{
	if (recording_controller)
		obs_frontend_remove_event_callback(frontend_event, recording_controller.get());
	recording_controller.reset();
	delete agent_client.data();
	delete confirmation_dialog.data();
	delete onboarding_dialog.data();
	tools_action.clear();
}
