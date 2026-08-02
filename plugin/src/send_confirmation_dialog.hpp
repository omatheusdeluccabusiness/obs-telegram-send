#pragma once

#include "agent_client.hpp"
#include "recording_controller.hpp"

#include <QDialog>

#include <cstdint>
#include <deque>
#include <functional>

class QCheckBox;
class QLabel;
class QPushButton;
class QProgressBar;
class QTimer;
class QWidget;

class SendConfirmationDialog : public QDialog
{
      public:
	using ConfirmationToken = std::uint64_t;
	using SendConfirmedHandler = std::function<void(const RecordingMetadata &, ConfirmationToken)>;

	explicit SendConfirmationDialog(QWidget *parent);
	SendConfirmationDialog(AgentClient *agent, QWidget *parent);

	ConfirmationToken show_for(RecordingMetadata metadata);
	void set_send_confirmed_handler(SendConfirmedHandler handler);
	void set_agent_ready(bool ready);
	void show_job_created(ConfirmationToken token, CreatedJobResult result);
	void apply_job_status(JobStatusResult status);
	bool send_button_enabled() const;

      private:
	enum class State { Idle, AwaitingConsent, CreatingJob, TrackingJob, Terminal };
	struct PendingConfirmation {
		ConfirmationToken token;
		RecordingMetadata metadata;
	};

	void display_confirmation(PendingConfirmation confirmation);
	void display_next_confirmation();
	void update_send_button();
	void poll_job();
	void show_request_error(const QString &message);
	bool is_terminal_poll_error(const AgentResult &result) const;

	AgentClient *agent_ = nullptr;
	RecordingMetadata metadata_;
	SendConfirmedHandler send_confirmed_;
	bool agent_ready_ = false;
	bool poll_in_flight_ = false;
	int consecutive_poll_failures_ = 0;
	ConfirmationToken next_confirmation_token_ = 0;
	ConfirmationToken active_confirmation_token_ = 0;
	State state_ = State::Idle;
	std::deque<PendingConfirmation> pending_confirmations_;
	QString current_job_id_;
	QLabel *name_label_ = nullptr;
	QLabel *duration_label_ = nullptr;
	QLabel *size_label_ = nullptr;
	QLabel *readiness_label_ = nullptr;
	QLabel *job_filename_label_ = nullptr;
	QLabel *job_message_label_ = nullptr;
	QCheckBox *consent_check_box_ = nullptr;
	QPushButton *send_button_ = nullptr;
	QPushButton *retry_button_ = nullptr;
	QProgressBar *job_progress_ = nullptr;
	QTimer *poll_timer_ = nullptr;
};
