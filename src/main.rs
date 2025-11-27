use iced::highlighter;
use iced::widget::{
    button, column, container, horizontal_space, pick_list, row, text, text_editor, tooltip,
};
use iced::{Element, Font, Length, Task, Theme, keyboard};

use std::ffi;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct Editor {
    path: Option<PathBuf>,
    content: text_editor::Content,
    error: Option<Error>,
    theme: highlighter::Theme,
    is_dirty: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    New,
    Open,
    FileOpened(Result<(PathBuf, Arc<String>), Error>),
    Save,
    FileSaved(Result<PathBuf, Error>),
    ThemeSelected(highlighter::Theme),
}

impl Editor {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                path: None,
                content: text_editor::Content::new(),
                error: None,
                theme: highlighter::Theme::SolarizedDark,
                is_dirty: true,
            },
            Task::perform(load_file(default_file()), Message::FileOpened),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(action) => {
                self.is_dirty = self.is_dirty || action.is_edit();
                self.error = None;
                self.content.perform(action);
                Task::none()
            }
            Message::New => {
                self.path = None;
                self.content = text_editor::Content::new();
                self.is_dirty = true;
                Task::none()
            }
            Message::Save => {
                let text = self.content.text();

                Task::perform(save_file(self.path.clone(), text), Message::FileSaved)
            }
            Message::Open => Task::perform(choose_file(), Message::FileOpened),
            Message::FileOpened(Ok((path, content))) => {
                self.path = Some(path);
                self.content = text_editor::Content::with_text(&content);
                self.is_dirty = false;
                Task::none()
            }
            Message::FileSaved(Ok(path)) => {
                self.path = Some(path);
                self.is_dirty = false;
                Task::none()
            }
            Message::FileSaved(Err(error)) => {
                self.error = Some(error);
                Task::none()
            }
            Message::FileOpened(Err(error)) => {
                self.error = Some(error);
                Task::none()
            }
            Message::ThemeSelected(theme) => {
                self.theme = theme;
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let controls = row![
            action(new_icon(), "New file", Some(Message::New)),
            action(open_icon(), "Open file", Some(Message::Open)),
            action(
                save_icon(),
                "Save file",
                self.is_dirty.then_some(Message::Save)
            ),
            horizontal_space(),
            pick_list(
                highlighter::Theme::ALL,
                Some(self.theme),
                Message::ThemeSelected
            )
        ]
        .spacing(10);

        let input = text_editor(&self.content)
            .on_action(Message::Edit)
            .height(Length::Fill)
            .highlight(
                self.path
                    .as_deref()
                    .and_then(Path::extension)
                    .and_then(ffi::OsStr::to_str)
                    .unwrap_or("rs"),
                self.theme,
            )
            .key_binding(|key_press| match key_press.key.as_ref() {
                keyboard::Key::Character("s") if key_press.modifiers.command() => {
                    Some(text_editor::Binding::Custom(Message::Save))
                }
                _ => text_editor::Binding::from_key_press(key_press),
            });

        let status_bar = {
            let status = if let Some(Error::IOFailed(error)) = self.error.as_ref() {
                text(error.to_string())
            } else {
                match self.path.as_deref().and_then(Path::to_str) {
                    Some(path) => text(path).size(14),
                    None => text("New file"),
                }
            };

            let position = {
                let (line, column) = self.content.cursor_position();

                text(format!("{}:{}", line + 1, column + 1))
            };

            row![status, horizontal_space().width(Length::Fill), position]
        };

        container(column![controls, input, status_bar].spacing(10))
            .padding(10)
            .into()
    }

    fn theme(&self) -> Theme {
        if self.theme.is_dark() {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}

#[derive(Debug, Clone)]
enum Error {
    DialogClosed,
    IOFailed(io::ErrorKind),
}

fn default_file() -> PathBuf {
    PathBuf::from(format!("{}/src/main.rs", env!("CARGO_MANIFEST_DIR")))
}

async fn choose_file() -> Result<(PathBuf, Arc<String>), Error> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Choose a text file...")
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?;

    load_file(handle.path().to_owned()).await
}

async fn load_file(path: PathBuf) -> Result<(PathBuf, Arc<String>), Error> {
    let content = tokio::fs::read_to_string(&path)
        .await
        .map(Arc::new)
        .map_err(|error| error.kind())
        .map_err(Error::IOFailed)?;

    Ok((path, content))
}

async fn save_file(path: Option<PathBuf>, text: String) -> Result<PathBuf, Error> {
    let path = if let Some(path) = path {
        path
    } else {
        rfd::AsyncFileDialog::new()
            .set_title("Choose a file name...")
            .save_file()
            .await
            .ok_or(Error::DialogClosed)
            .map(|handle| handle.path().to_owned())?
    };

    tokio::fs::write(&path, text)
        .await
        .map_err(|error| Error::IOFailed(error.kind()))?;

    Ok(path)
}

fn action<'a>(
    content: Element<'a, Message>,
    label: &'a str,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let action = button(container(content).width(30).center_x(30)).padding([5, 10]);

    if let Some(on_press) = on_press {
        tooltip(
            action.on_press(on_press),
            label,
            tooltip::Position::FollowCursor,
        )
        .style(container::rounded_box)
        .into()
    } else {
        action.style(button::secondary).into()
    }
}

fn new_icon<'a>() -> Element<'a, Message> {
    icon('\u{E800}')
}

fn open_icon<'a>() -> Element<'a, Message> {
    icon('\u{F115}')
}

fn save_icon<'a>() -> Element<'a, Message> {
    icon('\u{E801}')
}

fn icon<'a, Message>(codepoint: char) -> Element<'a, Message> {
    const ICON_FONT: Font = Font::with_name("editor-icons");

    text(codepoint).font(ICON_FONT).into()
}

fn main() -> iced::Result {
    iced::application("Text editor", Editor::update, Editor::view)
        .theme(Editor::theme)
        .font(include_bytes!("../res/icons.ttf").as_slice())
        .default_font(Font::MONOSPACE)
        .run_with(Editor::new)
}
