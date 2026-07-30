mod overview;
mod statics;

pub use overview::get_overview_page;
pub use statics::get_static_asset;

use std::sync::LazyLock;

use minijinja::Environment;
use minijinja_autoreload::AutoReloader;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../frontend/templates"]
struct TemplateAssets;

static VIEW_ENVIRONMENT: LazyLock<AutoReloader> = LazyLock::new(|| {
    AutoReloader::new(|notifier| {
        // In debug builds, always trigger a reload from the filesystem.
        #[cfg(debug_assertions)]
        notifier.set_callback(|| true);

        let templates = TemplateAssets::iter().filter_map(|name| {
            let file = TemplateAssets::get(&name)?;
            let template = std::str::from_utf8(&file.data)
                .expect(&format!(
                    "the template `{name}` should be a valid UTF-8 file"
                ))
                .to_string();
            Some((name.to_string(), template))
        });

        let mut env = Environment::new();
        for (name, template) in templates {
            env.add_template_owned(name, template).unwrap();
        }
        Ok(env)
    })
});
