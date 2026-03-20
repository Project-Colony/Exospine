mod accounts;
mod calendar;
mod categories;
mod config;
mod contacts;
mod import_export;
mod mail;
mod message;
mod search;
mod shortcuts;
mod state;
mod storage;
mod threading;
mod ui;
mod update;

use iced::{keyboard, Element, Size, Subscription, Task, Theme};
use iced::event;
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
        .subscription(App::subscription)
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

    fn view(&self) -> Element<'_, Message> {
        use iced::widget::{column, container, row, Stack};
        use iced::Length;

        // Show onboarding overlay if this is the first launch.
        if self.first_launch {
            return ui::onboarding::view(self);
        }

        // Base layout
        let base: Element<Message> = match self.view {
            state::View::Mail => {
                let sidebar = ui::sidebar::view(self);
                let mail_list = ui::mail_list::view(self);
                let mail_view = ui::mail_view::view(self);

                let content = row![sidebar, mail_list, mail_view];
                let mut layout = column![content.height(Length::Fill)];

                if let Some(ref status) = self.status_message {
                    layout = layout.push(
                        container(iced::widget::text(status.as_str()).size(12))
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
            state::View::Calendar => {
                let sidebar = ui::sidebar::view(self);
                let placeholder = container(
                    iced::widget::text("Calendar").size(20),
                )
                .padding(20)
                .width(Length::Fill)
                .height(Length::Fill);
                let content = row![sidebar, placeholder];
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
            state::View::Contacts => {
                let sidebar = ui::sidebar::view(self);
                let placeholder = container(
                    iced::widget::text("Contacts").size(20),
                )
                .padding(20)
                .width(Length::Fill)
                .height(Length::Fill);
                let content = row![sidebar, placeholder];
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
        };

        // Layer overlays: account dialog, confirm dialog, toasts
        let mut layers: Vec<Element<Message>> = vec![base];

        if let Some(ref dialog) = self.account_dialog {
            layers.push(ui::account_dialog::view(dialog));
        }

        if let Some(ref dialog) = self.confirm_dialog {
            layers.push(ui::confirm_dialog::view(dialog));
        }

        if let Some(ref menu) = self.context_menu {
            layers.push(ui::context_menu::view(menu, self));
        }

        if !self.toasts.is_empty() {
            layers.push(ui::toast::view(&self.toasts));
        }

        Stack::with_children(layers)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn subscription(&self) -> Subscription<Message> {
        let keyboard_sub = event::listen_with(|event, _status, _id| match event {
            iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                shortcuts::handle_key_event(key, modifiers)
            }
            _ => None,
        });

        let tick_sub = iced::time::every(std::time::Duration::from_secs(30))
            .map(|_| Message::Tick);

        Subscription::batch(vec![keyboard_sub, tick_sub])
    }
}
