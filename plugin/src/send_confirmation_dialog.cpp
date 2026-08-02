#include "send_confirmation_dialog.hpp"

#include <QCheckBox>
#include <QDialogButtonBox>
#include <QFormLayout>
#include <QLabel>
#include <QLocale>
#include <QProgressBar>
#include <QPushButton>
#include <QTimer>
#include <QVBoxLayout>

#include <algorithm>
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

SendConfirmationDialog::SendConfirmationDialog(QWidget *parent) : SendConfirmationDialog(nullptr, parent)
{
}

SendConfirmationDialog::SendConfirmationDialog(AgentClient *agent, QWidget *parent) : QDialog(parent), agent_(agent)
{
	setWindowTitle(tr("Enviar gravação ao Telegram"));
	setWindowModality(Qt::WindowModal);
	setModal(true);
	setMinimumWidth(440);

	auto *intro = new QLabel(tr("Sua gravação terminou. Confira os dados antes de enviar."), this);
	intro->setWordWrap(true);

	name_label_ = new QLabel(this);
	name_label_->setObjectName(QStringLiteral("recordingNameLabel"));
	duration_label_ = new QLabel(this);
	size_label_ = new QLabel(this);
	name_label_->setTextFormat(Qt::PlainText);
	duration_label_->setTextFormat(Qt::PlainText);
	size_label_->setTextFormat(Qt::PlainText);
	auto *details = new QFormLayout;
	details->addRow(tr("Arquivo:"), name_label_);
	details->addRow(tr("Duração:"), duration_label_);
	details->addRow(tr("Tamanho:"), size_label_);

	consent_check_box_ = new QCheckBox(tr("Enviar este vídeo ao Telegram"), this);
	consent_check_box_->setObjectName(QStringLiteral("sendConsentCheckBox"));
	readiness_label_ = new QLabel(this);
	readiness_label_->setObjectName(QStringLiteral("agentReadinessLabel"));
	readiness_label_->setTextFormat(Qt::PlainText);
	readiness_label_->setWordWrap(true);

	job_filename_label_ = new QLabel(this);
	job_filename_label_->setObjectName(QStringLiteral("jobFilenameLabel"));
	job_filename_label_->setTextFormat(Qt::PlainText);
	job_filename_label_->setVisible(false);
	job_progress_ = new QProgressBar(this);
	job_progress_->setObjectName(QStringLiteral("jobProgressBar"));
	job_progress_->setRange(0, 100);
	job_progress_->setValue(0);
	job_progress_->setVisible(false);
	job_message_label_ = new QLabel(this);
	job_message_label_->setObjectName(QStringLiteral("jobMessageLabel"));
	job_message_label_->setTextFormat(Qt::PlainText);
	job_message_label_->setWordWrap(true);
	job_message_label_->setVisible(false);
	retry_button_ = new QPushButton(tr("Tentar novamente"), this);
	retry_button_->setObjectName(QStringLiteral("retryJobButton"));
	retry_button_->setVisible(false);
	poll_timer_ = new QTimer(this);
	poll_timer_->setObjectName(QStringLiteral("jobPollTimer"));
	poll_timer_->setInterval(500);

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
	layout->addWidget(readiness_label_);
	layout->addWidget(consent_check_box_);
	layout->addWidget(job_filename_label_);
	layout->addWidget(job_progress_);
	layout->addWidget(job_message_label_);
	layout->addWidget(retry_button_);
	layout->addWidget(buttons);

	connect(consent_check_box_, &QCheckBox::toggled, this, [this] { update_send_button(); });
	connect(cancel_button, &QPushButton::clicked, this, [this] {
		reject();
		if (state_ != State::CreatingJob && state_ != State::TrackingJob) {
			state_ = State::Idle;
			display_next_confirmation();
		}
	});
	connect(send_button_, &QPushButton::clicked, this, [this] {
		if (state_ != State::AwaitingConsent || !agent_ready_ || !consent_check_box_->isChecked())
			return;
		state_ = State::CreatingJob;
		send_button_->setEnabled(false);
		consent_check_box_->setEnabled(false);
		job_filename_label_->setText(QString::fromStdString(metadata_.display_name));
		job_filename_label_->setVisible(true);
		job_message_label_->setText(tr("Solicitando o envio ao serviço local…"));
		job_message_label_->setVisible(true);
		job_progress_->setRange(0, 100);
		job_progress_->setValue(0);
		job_progress_->setVisible(true);
		if (send_confirmed_) {
			send_confirmed_(metadata_, active_confirmation_token_);
		} else {
			state_ = State::Terminal;
			show_request_error(tr("O envio não pôde ser iniciado."));
		}
	});
	connect(poll_timer_, &QTimer::timeout, this, [this] { poll_job(); });
	connect(retry_button_, &QPushButton::clicked, this, [this] {
		if (!agent_ || current_job_id_.isEmpty())
			return;
		retry_button_->setEnabled(false);
		job_message_label_->setText(tr("Tentando novamente…"));
		const QString requested_job_id = current_job_id_;
		agent_->retry_job(requested_job_id, [this, requested_job_id](JobStatusResult status) {
			if (requested_job_id != current_job_id_)
				return;
			retry_button_->setEnabled(true);
			apply_job_status(std::move(status));
		});
	});
	set_agent_ready(false);
}

SendConfirmationDialog::ConfirmationToken SendConfirmationDialog::show_for(RecordingMetadata metadata)
{
	PendingConfirmation confirmation{++next_confirmation_token_, std::move(metadata)};
	const auto token = confirmation.token;
	if (state_ == State::CreatingJob || state_ == State::TrackingJob) {
		pending_confirmations_.push_back(std::move(confirmation));
		return token;
	}
	display_confirmation(std::move(confirmation));
	return token;
}

void SendConfirmationDialog::display_confirmation(PendingConfirmation confirmation)
{
	active_confirmation_token_ = confirmation.token;
	metadata_ = std::move(confirmation.metadata);
	state_ = State::AwaitingConsent;
	name_label_->setText(QString::fromStdString(metadata_.display_name));
	duration_label_->setText(format_duration(metadata_.duration_ms));
	size_label_->setText(format_size(metadata_.size_bytes));
	consent_check_box_->setChecked(false);
	consent_check_box_->setEnabled(agent_ready_);
	current_job_id_.clear();
	poll_in_flight_ = false;
	consecutive_poll_failures_ = 0;
	poll_timer_->stop();
	poll_timer_->setInterval(500);
	job_filename_label_->setVisible(false);
	job_progress_->setVisible(false);
	job_message_label_->setVisible(false);
	retry_button_->setVisible(false);
	update_send_button();
	open();
	raise();
	activateWindow();

	if (agent_) {
		const auto token = active_confirmation_token_;
		set_agent_ready(false);
		agent_->probe([this, token](AgentResult) {
			if (token == active_confirmation_token_ && state_ == State::AwaitingConsent)
				set_agent_ready(agent_->is_ready());
		});
	}
}

void SendConfirmationDialog::display_next_confirmation()
{
	if (pending_confirmations_.empty())
		return;
	auto confirmation = std::move(pending_confirmations_.front());
	pending_confirmations_.pop_front();
	display_confirmation(std::move(confirmation));
}

void SendConfirmationDialog::set_agent_ready(bool ready)
{
	agent_ready_ = ready;
	if (consent_check_box_)
		consent_check_box_->setEnabled(ready && state_ == State::AwaitingConsent);
	if (readiness_label_) {
		readiness_label_->setText(ready ? tr("Serviço local conectado e configuração confirmada.")
		                                : tr("Envio indisponível. Clique em Não enviar; depois abra "
		                                     "Ferramentas → Telegram Send e conclua o teste final."));
	}
	update_send_button();
}

void SendConfirmationDialog::show_job_created(ConfirmationToken token, CreatedJobResult result)
{
	if (token != active_confirmation_token_ || state_ != State::CreatingJob)
		return;
	if (!result.result.ok) {
		state_ = State::Terminal;
		show_request_error(result.result.message);
		display_next_confirmation();
		return;
	}
	state_ = State::TrackingJob;
	current_job_id_ = result.job_id;
	poll_in_flight_ = false;
	consecutive_poll_failures_ = 0;
	poll_timer_->setInterval(500);
	job_message_label_->setText(tr("Envio colocado na fila…"));
	job_message_label_->setVisible(true);
	job_progress_->setRange(0, 100);
	job_progress_->setValue(0);
	job_progress_->setVisible(true);
	poll_timer_->start();
	poll_job();
}

void SendConfirmationDialog::apply_job_status(JobStatusResult status)
{
	if (!status.result.ok) {
		if (state_ != State::TrackingJob)
			return;
		if (is_terminal_poll_error(status.result)) {
			state_ = State::Terminal;
			poll_timer_->stop();
			show_request_error(status.result.message);
			return;
		}
		++consecutive_poll_failures_;
		const int exponent = std::min(consecutive_poll_failures_, 4);
		poll_timer_->setInterval(std::min(500 * (1 << exponent), 5000));
		job_message_label_->setText(tr("Conexão temporariamente indisponível. Tentando novamente…"));
		job_message_label_->setVisible(true);
		if (!poll_timer_->isActive())
			poll_timer_->start();
		return;
	}
	if (status.job_id != current_job_id_)
		return;
	consecutive_poll_failures_ = 0;
	poll_timer_->setInterval(500);
	job_filename_label_->setText(status.filename);
	job_filename_label_->setVisible(true);
	job_progress_->setVisible(true);
	if (status.progress_percent.has_value()) {
		job_progress_->setRange(0, 100);
		job_progress_->setValue(*status.progress_percent);
	} else if (status.state == QStringLiteral("uploading")) {
		job_progress_->setRange(0, 0);
	} else {
		job_progress_->setRange(0, 100);
		job_progress_->setValue(status.state == QStringLiteral("completed") ? 100 : 0);
	}

	QString message = status.message;
	if (message.isEmpty()) {
		if (status.state == QStringLiteral("queued"))
			message = tr("Aguardando o início do envio…");
		else if (status.state == QStringLiteral("uploading"))
			message = tr("Enviando ao Telegram…");
		else if (status.state == QStringLiteral("completed"))
			message = tr("Envio concluído. O arquivo original continua no computador.");
		else if (status.state == QStringLiteral("unknown"))
			message = tr("O resultado não pôde ser confirmado. Confira o Telegram "
			             "antes de qualquer nova tentativa.");
		else
			message = tr("O envio não foi concluído.");
	}
	job_message_label_->setText(message);
	job_message_label_->setVisible(true);

	const bool active = status.state == QStringLiteral("queued") || status.state == QStringLiteral("uploading");
	if (active) {
		state_ = State::TrackingJob;
		if (!poll_timer_->isActive())
			poll_timer_->start();
	} else {
		state_ = State::Terminal;
		poll_timer_->stop();
	}
	retry_button_->setVisible(status.retryable && status.state == QStringLiteral("failed"));
	if (status.state == QStringLiteral("completed"))
		display_next_confirmation();
}

void SendConfirmationDialog::update_send_button()
{
	if (send_button_)
		send_button_->setEnabled(state_ == State::AwaitingConsent && agent_ready_ && current_job_id_.isEmpty() &&
		                         consent_check_box_->isChecked() &&
		                         consent_check_box_->isEnabled());
}

void SendConfirmationDialog::poll_job()
{
	if (!agent_ || current_job_id_.isEmpty() || poll_in_flight_)
		return;
	poll_in_flight_ = true;
	const QString requested_job_id = current_job_id_;
	agent_->get_job(requested_job_id, [this, requested_job_id](JobStatusResult status) {
		if (requested_job_id != current_job_id_)
			return;
		poll_in_flight_ = false;
		apply_job_status(std::move(status));
	});
}

void SendConfirmationDialog::show_request_error(const QString &message)
{
	poll_timer_->stop();
	job_message_label_->setText(message.isEmpty() ? tr("O envio não pôde ser iniciado.") : message);
	job_message_label_->setVisible(true);
	consent_check_box_->setEnabled(agent_ready_ && state_ == State::AwaitingConsent);
	update_send_button();
}

bool SendConfirmationDialog::is_terminal_poll_error(const AgentResult &result) const
{
	return result.code == QStringLiteral("invalid_response") || result.code == QStringLiteral("job_not_found") ||
	       result.code == QStringLiteral("unauthorized") ||
	       (result.http_status >= 400 && result.http_status < 500);
}

void SendConfirmationDialog::set_send_confirmed_handler(SendConfirmedHandler handler)
{
	send_confirmed_ = std::move(handler);
}

bool SendConfirmationDialog::send_button_enabled() const
{
	return send_button_->isEnabled();
}
