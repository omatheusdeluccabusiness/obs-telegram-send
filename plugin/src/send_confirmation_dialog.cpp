#include "send_confirmation_dialog.hpp"

#include <QCheckBox>
#include <QDialogButtonBox>
#include <QFormLayout>
#include <QLabel>
#include <QLocale>
#include <QPushButton>
#include <QVBoxLayout>

#include <utility>

namespace {

QString format_duration(std::uint64_t duration_ms)
{
	const auto total_seconds = duration_ms / 1000;
	const auto hours = total_seconds / 3600;
	const auto minutes = (total_seconds % 3600) / 60;
	const auto seconds = total_seconds % 60;
	if (hours > 0)
		return QStringLiteral("%1:%2:%3")
			.arg(hours)
			.arg(minutes, 2, 10, QLatin1Char('0'))
			.arg(seconds, 2, 10, QLatin1Char('0'));
	return QStringLiteral("%1:%2").arg(minutes).arg(seconds, 2, 10, QLatin1Char('0'));
}

QString format_size(std::uint64_t size_bytes)
{
	return QLocale().formattedDataSize(static_cast<qint64>(size_bytes), 1, QLocale::DataSizeTraditionalFormat);
}

} // namespace

SendConfirmationDialog::SendConfirmationDialog(QWidget *parent) : QDialog(parent)
{
	setWindowTitle(tr("Enviar gravação ao Telegram"));
	setWindowModality(Qt::WindowModal);
	setModal(true);
	setMinimumWidth(440);

	auto *intro = new QLabel(tr("Sua gravação terminou. Confira os dados antes de enviar."), this);
	intro->setWordWrap(true);

	name_label_ = new QLabel(this);
	duration_label_ = new QLabel(this);
	size_label_ = new QLabel(this);
	auto *details = new QFormLayout;
	details->addRow(tr("Arquivo:"), name_label_);
	details->addRow(tr("Duração:"), duration_label_);
	details->addRow(tr("Tamanho:"), size_label_);

	consent_check_box_ = new QCheckBox(tr("Enviar este vídeo ao Telegram"), this);
	consent_check_box_->setObjectName(QStringLiteral("sendConsentCheckBox"));

	send_button_ = new QPushButton(tr("Enviar agora"), this);
	send_button_->setObjectName(QStringLiteral("sendNowButton"));
	send_button_->setEnabled(false);
	auto *cancel_button = new QPushButton(tr("Não enviar"), this);
	auto *buttons = new QDialogButtonBox(this);
	buttons->addButton(cancel_button, QDialogButtonBox::RejectRole);
	buttons->addButton(send_button_, QDialogButtonBox::AcceptRole);

	auto *layout = new QVBoxLayout(this);
	layout->addWidget(intro);
	layout->addLayout(details);
	layout->addWidget(consent_check_box_);
	layout->addWidget(buttons);

	connect(consent_check_box_, &QCheckBox::toggled, send_button_, &QPushButton::setEnabled);
	connect(cancel_button, &QPushButton::clicked, this, &QDialog::reject);
	connect(send_button_, &QPushButton::clicked, this, [this] {
		if (!consent_check_box_->isChecked())
			return;
		send_button_->setEnabled(false);
		if (send_confirmed_)
			send_confirmed_(metadata_);
	});
}

void SendConfirmationDialog::show_for(RecordingMetadata metadata)
{
	metadata_ = std::move(metadata);
	name_label_->setText(QString::fromStdString(metadata_.display_name));
	duration_label_->setText(format_duration(metadata_.duration_ms));
	size_label_->setText(format_size(metadata_.size_bytes));
	consent_check_box_->setChecked(false);
	send_button_->setEnabled(false);
	open();
	raise();
	activateWindow();
}

void SendConfirmationDialog::set_send_confirmed_handler(SendConfirmedHandler handler)
{
	send_confirmed_ = std::move(handler);
}

bool SendConfirmationDialog::send_button_enabled() const
{
	return send_button_->isEnabled();
}
