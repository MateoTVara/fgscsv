use clap::Parser;
use serde::{Deserialize, Serialize};

mod config;
mod cli;

#[derive(Debug, Serialize, Deserialize)]
struct Product {
    id: String,
    nombre: String,
    categoria: Option<String>,
    precio: f32,
    descripcion: String,
    img1: Option<String>,
    img2: Option<String>
}

struct Sheet {
    category: &'static str,
    gid: &'static str,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();

    match args.command {
        cli::Commands::Init => {
            cli::init()?;
        },
        cli::Commands::Set { key, value } => {
            cli::set(key, value)?;
        },
        cli::Commands::Run { output } => {
            let config = config::read_config()?;

            const SHEETS: [Sheet; 4] = [
                Sheet { category: "men's_clothing", gid: "39317466" },
                Sheet { category: "jewelery", gid: "154127679" },
                Sheet { category: "electronics", gid: "92713497" },
                Sheet { category: "women's_clothing", gid: "1909018467" },
            ];

            let client = reqwest::Client::new();
            let mut products: Vec<Product> = vec![];

            for sheet in SHEETS {
                run(&client, &config.spreadsheet_id, sheet.category, sheet.gid, &mut products).await?;
            }

            let output_path = output.unwrap_or(config.output_path);
            let output_path = std::path::PathBuf::from(output_path);

            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let f = std::fs::File::create(output_path)?;
            serde_json::to_writer_pretty(f, &products)?;
        }
    }

    Ok(())
}

async fn run(
    client: &reqwest::Client,
    spreadsheet_id: &str,
    category: &str,
    gid: &str,
    buffer: &mut Vec<Product>
) -> anyhow::Result<()> {
    let res = client.get(format!(
        "https://docs.google.com/spreadsheets/d/{}/export?format=csv&gid={}",
        spreadsheet_id, gid
    )).send().await?;
    
    let csv_content = res.text().await?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b',')
        .from_reader(csv_content.as_bytes());

    // println!("{:?}", rdr.headers()?);

    for result in rdr.deserialize() {
        let mut product: Product = result?;
        product.categoria = Some(category.to_string());
        // println!("{product:#?}");
        buffer.push(product);
    }

    Ok(())
}
