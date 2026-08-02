#pragma once

#include <QWizard>

#include <cstdint>

class AgentClient;
class QLabel;
class QLineEdit;
class QPushButton;

class OnboardingDialog : public QWizard
{
      public:
	explicit OnboardingDialog(AgentClient &agent, QWidget *parent = nullptr);
	void start_over();

      protected:
	bool validateCurrentPage() override;

      private:
	void detect_chat();
	void send_test();
	void set_busy(bool busy);
	void show_error(const QString &message);
	std::uint32_t api_id() const;

	AgentClient &agent_;
	QLineEdit *bot_token_ = nullptr;
	QLineEdit *api_id_ = nullptr;
	QLineEdit *api_hash_ = nullptr;
	QLabel *bot_status_ = nullptr;
	QLabel *access_status_ = nullptr;
	QLabel *chat_instruction_ = nullptr;
	QLabel *chat_status_ = nullptr;
	QLabel *test_status_ = nullptr;
	QPushButton *detect_button_ = nullptr;
	QPushButton *test_button_ = nullptr;
	QString challenge_;
	qint64 chat_id_ = 0;
	bool busy_ = false;
	bool test_succeeded_ = false;
};
