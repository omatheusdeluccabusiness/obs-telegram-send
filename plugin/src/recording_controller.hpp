#pragma once

#include <obs-frontend-api.h>

#include <cstdint>
#include <functional>
#include <optional>
#include <string>

struct RecordingMetadata {
	std::string path;
	std::string display_name;
	std::uint64_t duration_ms = 0;
	std::uint64_t size_bytes = 0;
};

class RecordingController {
public:
	using CreateJob = std::function<void(const std::string &, const std::string &)>;
	using ResolveRecording = std::function<std::optional<RecordingMetadata>()>;
	using ShowConfirmation = std::function<void(const RecordingMetadata &)>;

	RecordingController(CreateJob create_job, ResolveRecording resolve_recording,
			    ShowConfirmation show_confirmation);

	template<typename AgentClient>
	explicit RecordingController(AgentClient &agent)
		: create_job_([&agent](const std::string &path, const std::string &display_name) {
			  agent.create_job(path, display_name);
		  })
	{
	}

	void recording_finished(RecordingMetadata metadata);
	void confirm_send();
	void on_frontend_event(obs_frontend_event event);

private:
	CreateJob create_job_;
	ResolveRecording resolve_recording_;
	ShowConfirmation show_confirmation_;
	std::optional<RecordingMetadata> pending_recording_;
};
