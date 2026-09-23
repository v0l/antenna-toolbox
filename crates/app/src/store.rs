#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::path::PathBuf;

    fn file(name: &str) -> Option<PathBuf> {
        Some(dirs::config_dir()?.join("antenna-toolbox").join(name))
    }

    pub fn read(name: &str) -> Option<String> {
        std::fs::read_to_string(file(name)?).ok()
    }

    pub fn write(name: &str, text: &str) {
        let Some(path) = file(name) else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, text);
    }

    pub fn remove(name: &str) {
        if let Some(path) = file(name) {
            let _ = std::fs::remove_file(path);
        }
    }

    pub fn download(file: &str, text: &str) -> String {
        let dir = dirs::download_dir().or_else(dirs::home_dir).unwrap_or_default();
        let path = dir.join(file);
        match std::fs::write(&path, text) {
            Ok(()) => format!("wrote {}", path.display()),
            Err(e) => format!("could not write {}: {e}", path.display()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use wasm_bindgen::JsCast;

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    fn key(name: &str) -> String {
        format!("antenna-toolbox/{name}")
    }

    pub fn read(name: &str) -> Option<String> {
        storage()?.get_item(&key(name)).ok()?
    }

    pub fn write(name: &str, text: &str) {
        if let Some(s) = storage() {
            let _ = s.set_item(&key(name), text);
        }
    }

    pub fn remove(name: &str) {
        if let Some(s) = storage() {
            let _ = s.remove_item(&key(name));
        }
    }

    pub fn download(file: &str, text: &str) -> String {
        let go = || -> Option<()> {
            let parts = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text));
            let options = web_sys::BlobPropertyBag::new();
            options.set_type("text/plain");
            let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options).ok()?;
            let url = web_sys::Url::create_object_url_with_blob(&blob).ok()?;
            let document = web_sys::window()?.document()?;
            let a =
                document.create_element("a").ok()?.dyn_into::<web_sys::HtmlAnchorElement>().ok()?;
            a.set_href(&url);
            a.set_download(file);
            a.click();
            let _ = web_sys::Url::revoke_object_url(&url);
            Some(())
        };
        match go() {
            Some(()) => format!("downloaded {file}"),
            None => format!("the browser would not save {file}"),
        }
    }
}

pub use imp::{download, read, remove, write};
