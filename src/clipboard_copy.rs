//! Records our writes even when the clipboard backend uses a null owner window.
use eframe::egui;
use std::borrow::Cow;
use std::sync::Mutex;

pub(crate) static LAST_WRITE: Mutex<u32> = Mutex::new(0);

pub struct Clipboard(arboard::Clipboard);

impl Clipboard {
    pub fn new() -> Result<Self, arboard::Error> {
        arboard::Clipboard::new().map(Self)
    }

    fn write(
        &mut self,
        f: impl FnOnce(&mut arboard::Clipboard) -> Result<(), arboard::Error>,
    ) -> Result<(), arboard::Error> {
        let mut sequence = LAST_WRITE.lock().unwrap_or_else(|e| e.into_inner());
        let result = f(&mut self.0);
        #[cfg(windows)]
        if result.is_ok() {
            // SAFETY: reading the clipboard sequence requires no handles.
            *sequence =
                unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() };
        }
        #[cfg(not(windows))]
        let _ = &mut sequence;
        result
    }

    pub fn set_text<'a>(&mut self, text: impl Into<Cow<'a, str>>) -> Result<(), arboard::Error> {
        self.write(|c| c.set_text(text))
    }
    pub fn set_html<'a>(
        &mut self,
        html: impl Into<Cow<'a, str>>,
        alt: Option<impl Into<Cow<'a, str>>>,
    ) -> Result<(), arboard::Error> {
        self.write(|c| c.set_html(html.into(), alt.map(Into::into)))
    }
    pub fn set_image(&mut self, image: arboard::ImageData<'_>) -> Result<(), arboard::Error> {
        self.write(|c| c.set_image(image))
    }
}

impl std::ops::Deref for Clipboard {
    type Target = arboard::Clipboard;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for Clipboard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub fn install(ctx: &egui::Context) {
    ctx.on_end_pass(
        "record_own_clipboard",
        std::sync::Arc::new(|ctx| {
            ctx.output_mut(|out| {
                out.commands.retain(|command| {
                    let result = match command {
                        egui::OutputCommand::CopyText(text) => {
                            Clipboard::new().and_then(|mut c| c.set_text(text.as_str()))
                        }
                        egui::OutputCommand::CopyImage(image) => {
                            let bytes: Vec<u8> =
                                image.pixels.iter().flat_map(|p| p.to_array()).collect();
                            Clipboard::new().and_then(|mut c| {
                                c.set_image(arboard::ImageData {
                                    width: image.width(),
                                    height: image.height(),
                                    bytes: Cow::Owned(bytes),
                                })
                            })
                        }
                        _ => return true,
                    };
                    // Retain failed writes so the platform integration can retry.
                    result.is_err()
                });
            });
        }),
    );
}
