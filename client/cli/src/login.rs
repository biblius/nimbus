use crate::{
    api::{Api, LoginResult},
    config,
};
use anyhow::{Context, Result, bail};
use std::io::{self, Write};

pub async fn run(server_arg: Option<&str>) -> Result<()> {
    let server = match server_arg {
        Some(server) => config::normalize_server(server)?,
        None => match config::load_server()? {
            Some(saved) => config::normalize_server(&saved)?,
            None => {
                print!("Server URL: ");
                io::stdout().flush().ok();
                let mut line = String::new();
                io::stdin()
                    .read_line(&mut line)
                    .context("unable to read server URL")?;
                config::normalize_server(line.trim())?
            }
        },
    };

    print!("Username: ");

    io::stdout().flush().ok();

    let mut username = String::new();

    io::stdin()
        .read_line(&mut username)
        .context("unable to read username")?;

    let username = username.trim();

    if username.is_empty() {
        bail!("username cannot be empty");
    }

    let password = rpassword::prompt_password("Password: ").context("unable to read password")?;

    if password.is_empty() {
        bail!("password cannot be empty");
    }

    // Login itself is unauthenticated; token is obtained from the response.
    let api = Api::new(&server, None)?;

    let result = api.login(username, &password, true).await?;

    let session_id = match result {
        LoginResult::Session { session_id } => session_id,
        LoginResult::TwoFactorRequired { token } => {
            let otp = rpassword::prompt_password("OTP code: ").context("unable to read OTP")?;

            let otp = otp.trim();

            if otp.is_empty() {
                bail!("OTP code cannot be empty");
            }

            match api.login_otp(&token, otp).await? {
                LoginResult::Session { session_id } => session_id,
                LoginResult::TwoFactorRequired { .. } => {
                    bail!("server still requires two-factor authentication");
                }
            }
        }
    };

    config::save_server(&server)?;

    config::save_token(&session_id.to_string())?;

    println!("Logged in to {server}");

    Ok(())
}
