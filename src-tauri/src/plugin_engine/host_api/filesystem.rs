use super::*;

pub(super) fn inject_fs<'js>(ctx: &Ctx<'js>, host: &Object<'js>) -> rquickjs::Result<()> {
    let fs_obj = Object::new(ctx.clone())?;

    fs_obj.set(
        "exists",
        Function::new(ctx.clone(), move |path: String| -> bool {
            let expanded = expand_path(&path);
            std::path::Path::new(&expanded).exists()
        })?,
    )?;

    fs_obj.set(
        "readText",
        Function::new(
            ctx.clone(),
            move |ctx_inner: Ctx<'_>, path: String| -> rquickjs::Result<String> {
                let expanded = expand_path(&path);
                std::fs::read_to_string(&expanded)
                    .map_err(|e| Exception::throw_message(&ctx_inner, &e.to_string()))
            },
        )?,
    )?;

    fs_obj.set(
        "writeText",
        Function::new(
            ctx.clone(),
            move |ctx_inner: Ctx<'_>, path: String, content: String| -> rquickjs::Result<()> {
                let expanded = expand_path(&path);
                std::fs::write(&expanded, &content)
                    .map_err(|e| Exception::throw_message(&ctx_inner, &e.to_string()))
            },
        )?,
    )?;

    fs_obj.set(
        "listDir",
        Function::new(
            ctx.clone(),
            move |ctx_inner: Ctx<'_>, path: String| -> rquickjs::Result<Vec<String>> {
                let expanded = expand_path(&path);
                let entries = std::fs::read_dir(&expanded)
                    .map_err(|e| Exception::throw_message(&ctx_inner, &e.to_string()))?;

                let mut names = Vec::new();
                for entry in entries {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(_) => continue,
                    };
                    let name_os = entry.file_name();
                    let name = name_os.to_string_lossy().to_string();
                    if !name.is_empty() {
                        names.push(name);
                    }
                }
                names.sort();
                Ok(names)
            },
        )?,
    )?;

    host.set("fs", fs_obj)?;
    Ok(())
}
