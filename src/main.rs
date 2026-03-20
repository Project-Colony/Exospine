mod config;
mod mail;
mod message;
mod state;
mod storage;
mod ui;
mod update;

use iced::{Element, Size, Task, Theme};
use message::Message;
use state::App;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("exospine=info".parse().unwrap()),
        )
        .init();

    tracing::info!("Starting Exospine");

    iced::application(App::new, App::update, App::view)
        .title("Exospine")
        .theme(App::theme)
        .window_size(Size::new(1200.0, 800.0))
        .run()
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let app = Self::default();
        (app, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        update::update(self, message)
    }

    fn view(&self) -> Element<Message> {
        use iced::widget::{column, container, row, text};
        use iced::Length;

        match self.view {
            state::View::Mail => {
                let sidebar = ui::sidebar::view(self);
                let mail_list = ui::mail_list::view(self);
                let mail_view = ui::mail_view::view(self);

                let content = row![sidebar, mail_list, mail_view];

                let mut layout = column![content.height(Length::Fill)];

                if let Some(ref status) = self.status_message {
                    layout = layout.push(
                        container(text(status.as_str()).size(12))
                            .padding(4)
                            .width(Length::Fill),
                    );
                }

                container(layout)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
            state::View::Compose => {
                let sidebar = ui::sidebar::view(self);
                let compose = ui::compose::view(self);

                let content = row![sidebar, compose];

                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
            state::View::Settings => {
                let sidebar = ui::sidebar::view(self);
                let settings = ui::settings::view(self);

                let content = row![sidebar, settings];

                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
        }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}
