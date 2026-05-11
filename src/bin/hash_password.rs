use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use rand::rngs::OsRng;
use std::io::{self, Write};

fn main() {
    println!("ostinato-hash-password: Generate an Argon2 hash for APP_PASSWORD_HASH");
    println!("WARNING: The password you type will be visible in the terminal.");
    println!(
        "         Consider piping: echo -n 'yourpassword' | cargo run --bin ostinato-hash-password"
    );
    println!();

    print!("Enter password: ");
    io::stdout().flush().unwrap();

    let mut password = String::new();
    io::stdin().read_line(&mut password).unwrap();
    let password = password.trim_end(); // remove trailing newline

    if password.is_empty() {
        eprintln!("Error: password cannot be empty");
        std::process::exit(1);
    }

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .expect("Failed to hash password");

    println!();
    println!("Hash (copy this into your .env as APP_PASSWORD_HASH):");
    println!("{}", password_hash);
    println!();
    println!("Example .env entry:");
    println!("APP_PASSWORD_HASH='{}'", password_hash);
    println!();
    println!("Also set JWT_SECRET to any long random string:");
    println!("JWT_SECRET='change-me-to-a-long-random-string'");
}
