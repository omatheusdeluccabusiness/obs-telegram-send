#pragma once

#include "agent_client.hpp"
#include "recording_controller.hpp"

#include <QDialog>

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
	using SendConfirmedHandler = std::function<void(const RecordingMetadata &)>;

	explicit SendConfirmationDialog(QWidget *parent);
	SendConfirmationDialog(AgentClient *agent, QWidget *parent);

	void show_for(RecordingMetadata metadata);
	void set_send_confirmed_handler(SendConfirmedHandler handler);
	void set_agent_ready(bool ready);
	void show_job_created(CreatedJobResult result);
	void apply_job_status(JobStatusResult status);
	bool send_button_enabled() const;

      private:
	void update_send_button();
	void poll_job();
	void show_request_error(const QString &message);

	AgentClient *agent_ = nullptr;
	RecordingMetadata metadata_;
	SendConfirmedHandler send_confirmed_;
	bool agent_ready_ = false;
	bool poll_in_flight_ = false;
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
