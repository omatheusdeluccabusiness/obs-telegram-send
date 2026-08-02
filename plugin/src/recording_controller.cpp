#include "recording_controller.hpp"

#include <utility>

RecordingController::RecordingController(CreateJob create_job, ResolveRecording resolve_recording,
					 ShowConfirmation show_confirmation)
	: create_job_(std::move(create_job)),
	  resolve_recording_(std::move(resolve_recording)),
	  show_confirmation_(std::move(show_confirmation))
{
}

void RecordingController::recording_finished(RecordingMetadata metadata)
{
	pending_recording_ = std::move(metadata);
	if (show_confirmation_)
		show_confirmation_(*pending_recording_);
}

void RecordingController::confirm_send()
{
	if (!pending_recording_)
		return;

	auto confirmed = std::move(*pending_recording_);
	pending_recording_.reset();
	create_job_(confirmed.path, confirmed.display_name);
}

void RecordingController::on_frontend_event(obs_frontend_event event)
{
	if (event != OBS_FRONTEND_EVENT_RECORDING_STOPPED || !resolve_recording_)
		return;

	auto recording = resolve_recording_();
	if (recording)
		recording_finished(std::move(*recording));
}
