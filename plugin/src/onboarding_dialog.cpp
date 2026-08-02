#include "onboarding_dialog.hpp"

#include "agent_client.hpp"

#include <QAbstractButton>
#include <QDesktopServices>
#include <QFormLayout>
#include <QLabel>
#include <QLineEdit>
#include <QPushButton>
#include <QRandomGenerator>
#include <QUrl>
#include <QVBoxLayout>
#include <QWizardPage>

namespace {

QLabel *instruction(const QString &text, QWidget *parent)
{
	auto *label = new QLabel(text, parent);
	label->setTextFormat(Qt::PlainText);
	label->setWordWrap(true);
	return label;
}

QPushButton *external_link(const QString &text, const QUrl &url, const QString &object_name, QWidget *parent)
{
	auto *button = new QPushButton(text, parent);
	button->setObjectName(object_name);
	button->setProperty("targetUrl", url);
	QObject::connect(button, &QPushButton::clicked, button, [url] { QDesktopServices::openUrl(url); });
	return button;
}

} // namespace

OnboardingDialog::OnboardingDialog(AgentClient &agent, QWidget *parent) : QWizard(parent), agent_(agent)
{
	setWindowTitle(tr("Telegram Send"));
	setWizardStyle(QWizard::ModernStyle);
	setMinimumSize(620, 430);
	setOption(QWizard::NoBackButtonOnStartPage);

	auto *bot_page = new QWizardPage(this);
	bot_page->setTitle(tr("Seu bot"));
	bot_page->setSubTitle(tr("Crie um bot só para receber suas gravações."));
	bot_token_ = new QLineEdit(bot_page);
	bot_token_->setObjectName(QStringLiteral("botTokenField"));
	bot_token_->setEchoMode(QLineEdit::Password);
	bot_token_->setPlaceholderText(tr("Cole aqui o token enviado pelo BotFather"));
	auto *bot_layout = new QVBoxLayout(bot_page);
	bot_layout->addWidget(instruction(tr("1. Abra o BotFather.\n2. Envie /newbot e siga as "
	                                     "instruções.\n3. Copie o token que ele mostrar."),
	                                  bot_page));
	bot_layout->addWidget(external_link(tr("Abrir BotFather"), QUrl(QStringLiteral("https://t.me/BotFather")),
	                                    QStringLiteral("openBotFatherButton"), bot_page));
	bot_layout->addWidget(bot_token_);
	bot_status_ = instruction({}, bot_page);
	bot_layout->addWidget(bot_status_);
	bot_layout->addStretch();
	addPage(bot_page);

	auto *access_page = new QWizardPage(this);
	access_page->setTitle(tr("Seu acesso ao Telegram"));
	access_page->setSubTitle(tr("Esses dois dados permitem enviar arquivos grandes pelo serviço local."));
	api_id_ = new QLineEdit(access_page);
	api_id_->setObjectName(QStringLiteral("apiIdField"));
	api_id_->setInputMethodHints(Qt::ImhDigitsOnly);
	api_hash_ = new QLineEdit(access_page);
	api_hash_->setObjectName(QStringLiteral("apiHashField"));
	api_hash_->setEchoMode(QLineEdit::Password);
	auto *access_form = new QFormLayout;
	access_form->addRow(tr("api_id:"), api_id_);
	access_form->addRow(tr("api_hash:"), api_hash_);
	auto *access_layout = new QVBoxLayout(access_page);
	access_layout->addWidget(instruction(tr("1. Abra my.telegram.org e entre com seu telefone.\n2. "
	                                        "Abra “API development tools”.\n3. Crie um aplicativo e "
	                                        "copie api_id e api_hash."),
	                                     access_page));
	access_layout->addWidget(external_link(tr("Abrir my.telegram.org"),
	                                       QUrl(QStringLiteral("https://my.telegram.org")),
	                                       QStringLiteral("openTelegramAppButton"), access_page));
	access_layout->addLayout(access_form);
	access_status_ = instruction({}, access_page);
	access_layout->addWidget(access_status_);
	access_layout->addStretch();
	addPage(access_page);

	auto *chat_page = new QWizardPage(this);
	chat_page->setTitle(tr("Seu chat"));
	chat_page->setSubTitle(tr("Avise ao bot qual conversa receberá as gravações."));
	challenge_ = QString::number(QRandomGenerator::global()->generate64(), 16).rightJustified(16, QLatin1Char('0'));
	chat_instruction_ = instruction(tr("1. Abra uma conversa com o bot que você criou.\n2. Envie exatamente: "
	                                   "/start %1\n3. Volte aqui e clique em Detectar meu chat.")
	                                    .arg(challenge_),
	                                chat_page);
	chat_instruction_->setTextInteractionFlags(Qt::TextSelectableByMouse);
	detect_button_ = new QPushButton(tr("Detectar meu chat"), chat_page);
	detect_button_->setObjectName(QStringLiteral("detectChatButton"));
	chat_status_ = instruction({}, chat_page);
	auto *chat_layout = new QVBoxLayout(chat_page);
	chat_layout->addWidget(chat_instruction_);
	chat_layout->addWidget(detect_button_);
	chat_layout->addWidget(chat_status_);
	chat_layout->addStretch();
	connect(detect_button_, &QPushButton::clicked, this, [this] { detect_chat(); });
	addPage(chat_page);

	auto *test_page = new QWizardPage(this);
	test_page->setTitle(tr("Teste final"));
	test_page->setSubTitle(tr("Confirme a conexão antes de liberar o envio de gravações."));
	test_button_ = new QPushButton(tr("Enviar mensagem de teste"), test_page);
	test_button_->setObjectName(QStringLiteral("testSendButton"));
	test_status_ = instruction(tr("O botão “Concluir” só será liberado depois que a mensagem chegar."), test_page);
	auto *test_layout = new QVBoxLayout(test_page);
	test_layout->addWidget(instruction(tr("Clique abaixo. O Telegram deve receber uma mensagem "
	                                      "curta do OBS Telegram Send."),
	                                   test_page));
	test_layout->addWidget(test_button_);
	test_layout->addWidget(test_status_);
	test_layout->addStretch();
	connect(test_button_, &QPushButton::clicked, this, [this] { send_test(); });
	addPage(test_page);

	connect(this, &QWizard::currentIdChanged, this, [this](int) {
		if (button(QWizard::FinishButton))
			button(QWizard::FinishButton)->setEnabled(test_succeeded_ && !busy_);
	});
	connect(this, &QWizard::finished, this, [this] {
		bot_token_->clear();
		api_hash_->clear();
	});
}

void OnboardingDialog::start_over()
{
	bot_token_->clear();
	api_id_->clear();
	api_hash_->clear();
	bot_status_->clear();
	access_status_->clear();
	chat_status_->clear();
	test_status_->setText(tr("O botão “Concluir” só será liberado depois que a mensagem chegar."));
	chat_id_ = 0;
	test_succeeded_ = false;
	challenge_ = QString::number(QRandomGenerator::global()->generate64(), 16).rightJustified(16, QLatin1Char('0'));
	chat_instruction_->setText(tr("1. Abra uma conversa com o bot que você criou.\n2. Envie exatamente: "
	                              "/start %1\n3. Volte aqui e clique em Detectar meu chat.")
	                               .arg(challenge_));
	restart();
	set_busy(false);
}

bool OnboardingDialog::validateCurrentPage()
{
	if (busy_)
		return false;
	if (currentId() == 0 && bot_token_->text().trimmed().isEmpty()) {
		show_error(tr("Cole o token recebido do BotFather para continuar."));
		return false;
	}
	if (currentId() == 1 && (api_id() == 0 || api_hash_->text().trimmed().isEmpty())) {
		show_error(tr("Preencha api_id e api_hash para continuar."));
		return false;
	}
	if (currentId() == 2 && chat_id_ == 0) {
		show_error(tr("Detecte seu chat antes de continuar."));
		return false;
	}
	return currentId() != 3 || test_succeeded_;
}

void OnboardingDialog::detect_chat()
{
	if (bot_token_->text().trimmed().isEmpty() || api_id() == 0 || api_hash_->text().trimmed().isEmpty()) {
		show_error(tr("Volte e confira o token, api_id e api_hash."));
		return;
	}
	set_busy(true);
	chat_status_->setText(tr("Procurando o comando no seu chat…"));
	agent_.detect_chat(bot_token_->text().trimmed(), api_id(), api_hash_->text().trimmed(), challenge_,
	                   [this](ChatDetectionResult result) {
		                   set_busy(false);
		                   if (!result.result.ok) {
			                   chat_status_->setText(result.result.message);
			                   return;
		                   }
		                   chat_id_ = result.chat_id;
		                   chat_status_->setText(tr("Chat encontrado. Você já pode continuar."));
	                   });
}

void OnboardingDialog::send_test()
{
	if (chat_id_ == 0 || busy_)
		return;
	set_busy(true);
	test_status_->setText(tr("Salvando a conexão com segurança…"));
	agent_.save_config(bot_token_->text().trimmed(), api_id(), api_hash_->text().trimmed(), chat_id_,
	                   [this](AgentResult saved) {
		                   if (!saved.ok) {
			                   set_busy(false);
			                   test_status_->setText(saved.message);
			                   return;
		                   }
		                   test_status_->setText(tr("Enviando a mensagem de teste…"));
		                   agent_.test_send([this](AgentResult sent) {
			                   set_busy(false);
			                   if (!sent.ok) {
				                   test_status_->setText(sent.message);
				                   return;
			                   }
			                   test_succeeded_ = true;
			                   bot_token_->clear();
			                   api_hash_->clear();
			                   test_status_->setText(tr("Tudo certo. A mensagem chegou ao Telegram "
			                                            "e o envio está liberado."));
			                   if (button(QWizard::FinishButton))
				                   button(QWizard::FinishButton)->setEnabled(true);
		                   });
	                   });
}

void OnboardingDialog::set_busy(bool busy)
{
	busy_ = busy;
	detect_button_->setEnabled(!busy);
	test_button_->setEnabled(!busy);
	if (button(QWizard::BackButton))
		button(QWizard::BackButton)->setEnabled(!busy);
	if (button(QWizard::NextButton))
		button(QWizard::NextButton)->setEnabled(!busy);
	if (button(QWizard::FinishButton))
		button(QWizard::FinishButton)->setEnabled(!busy && test_succeeded_);
}

void OnboardingDialog::show_error(const QString &message)
{
	if (currentId() == 0)
		bot_status_->setText(message);
	else if (currentId() == 1)
		access_status_->setText(message);
	else if (currentId() == 2)
		chat_status_->setText(message);
	else if (currentId() == 3)
		test_status_->setText(message);
}

std::uint32_t OnboardingDialog::api_id() const
{
	bool valid = false;
	const auto value = api_id_->text().trimmed().toUInt(&valid);
	return valid ? value : 0;
}
