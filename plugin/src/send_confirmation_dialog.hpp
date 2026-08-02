#pragma once

#include "recording_controller.hpp"

#include <QDialog>

#include <functional>

class QCheckBox;
class QLabel;
class QPushButton;
class QWidget;

class SendConfirmationDialog : public QDialog {
public:
	using SendConfirmedHandler = std::function<void(const RecordingMetadata &)>;

	explicit SendConfirmationDialog(QWidget *parent);

	void show_for(RecordingMetadata metadata);
	void set_send_confirmed_handler(SendConfirmedHandler handler);
	bool send_button_enabled() const;

private:
	RecordingMetadata metadata_;
	SendConfirmedHandler send_confirmed_;
	QLabel *name_label_ = nullptr;
	QLabel *duration_label_ = nullptr;
	QLabel *size_label_ = nullptr;
	QCheckBox *consent_check_box_ = nullptr;
	QPushButton *send_button_ = nullptr;
};
