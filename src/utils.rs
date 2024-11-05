use color_eyre::eyre::{Context, Result};

pub fn get_env_var(key: &str) -> Result<String> {
    let value = std::env::var(key)
        .wrap_err_with(|| format!("failed to find key '{key}' in the environment variables"))?;

    Ok(value)
}
