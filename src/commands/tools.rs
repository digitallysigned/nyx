use crate::{serenity};
pub(crate) use crate::types::{Context, Data, Error};
use crate::tooling::image::{analyze, fetch};
use evalexpr::{eval_number_with_context, math_consts_context, ContextWithMutableFunctions, ContextWithMutableVariables};

pub fn all_commands() -> Vec<poise::Command<Data, Error>> {
    vec![
        inspectimage(),
        expr(),
        // add more here
    ]
}

#[poise::command(slash_command)]
pub async fn inspectimage(
    ctx: Context<'_>,
    image: Option<serenity::all::Attachment>,
    url: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?; // defer to close short time window for response

    let image_url = if let Some(image) = &image {
        image.url.clone()
    } else if let Some(url) = &url {
        url.clone()
    } else { return Err("Provide image/URL for inspection".into()) };

    let bytes = match fetch::from_url(&image_url).await {
        Ok(bytes) => bytes,

        Err(error) => {
            let message = match error {
                fetch::FetchError::InvalidUrl(_) => {
                    "The URL provided is invalid"
                }
                fetch::FetchError::InvalidScheme => {
                    "The URL provided is not hypertext"
                }
                fetch::FetchError::TooLarge => {
                    "The image provided is over the 150MB maximum"
                }
                fetch::FetchError::Timeout => {
                    "The remote server took too long to respond"
                }
                fetch::FetchError::Request(_) => {
                    "Image download for inspection failed"
                }
            };
            ctx.send(poise::CreateReply::default().content(message)).await?;

            return Ok(());
        }
    };

    let info = match analyze(&bytes) {
        Ok(info) => info,
        Err(error) => {
            ctx.send(poise::CreateReply::default()
                .content("File is not a supported image")).await?;
            return Ok(());
        }
    };

    let mut embed = serenity::CreateEmbed::default()
        .title("Image Information")
        .color(0x5865F2)
        .image(&image_url)
        .field("Image Format", format!("`{}`", info.format.extensions_str().join(", ")), true)
        .field("Size", format!("`{:.2}` MB", info.size as f64 / 1000000.0), true)
        .field("Resolution", format!("{} x {}", info.width, info.height), true)
        .field("Megapixels", format!("`{:.2} MP`", info.megapixels), true)
        .field("Aspect Ratio", format!("`{:.2}:1`", info.aspect_ratio), true)
        .field("Colour", format!("`{:?}`", info.color_type), true);

    if let Some(exif) = info.exif {
        embed = embed.field("EXIF Metadata", exif.to_text(), false);
    } else {
        embed = embed.field("EXIF", "No EXIF metadata found", false);
    }

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}


#[poise::command(slash_command)]
pub async fn expr(
    ctx: Context<'_>,
    #[description = "Mathematical expression to evaluate"]
    expression: String,
) -> Result<(), Error> {
    use evalexpr::{eval_number_with_context, math_consts_context, DefaultNumericTypes, HashMapContext, Function, Value};

    ctx.defer().await?;

    let mut context: HashMapContext<DefaultNumericTypes> = match math_consts_context!() {
        Ok(context) => context,
        Err(error) => {
            ctx.send(poise::CreateReply::default()
                .content(format!("Failed to initialise calculator: `{}`", error))
            ).await?;

            return Ok(());
        }
    };

    context.set_function(
        "log".into(),
        Function::new(|arg| -> Result<Value, evalexpr::EvalexprError> {
            let value = arg.as_number()?;
            Ok(Value::from_float(value.log10()))
        }),
    )?;

    context.set_function(
        "ln".into(),
        Function::new(|arg| -> Result<Value, evalexpr::EvalexprError> {
            let value = arg.as_number()?;
            Ok(Value::from_float(value.ln()))
        }),
    )?;
    context.set_function(
        "logn".into(),
        Function::new(|arg| -> Result<Value, evalexpr::EvalexprError> {
            let args = arg.as_tuple()?;
            if args.len() != 2 {
                return Err(evalexpr::EvalexprError::wrong_function_argument_amount(
                    2, args.len()
                ));
            }

            let value = args[0].as_number()?;
            let base = args[1].as_number()?;

            Ok(Value::from_float(value.log(base)))
        }),
    )?;

    context.set_value("e".into(), Value::from_float(std::f64::consts::E))?;
    context.set_value("pi".into(), Value::from_float(std::f64::consts::PI))?;
    context.set_value("tau".into(), Value::from_float(std::f64::consts::TAU))?;
    context.set_value("phi".into(), Value::from_float(1.618033988749895))?;
    context.set_value("sqrt2".into(), Value::from_float(std::f64::consts::SQRT_2))?;

    let result: Result<f64, _> = eval_number_with_context(&expression, &context);

    let result = match result {
        Ok(result) => result,
        Err(error) => {
            ctx.send(poise::CreateReply::default()
                .content(format!("Invalid expression: `{}`", error))
            ).await?;

            return Ok(());
        }
    };

    if !result.is_finite() {
        ctx.send(poise::CreateReply::default()
            .content("Expression produced an invalid result (NaN or infinity)")
        ).await?;

        return Ok(());
    }

    ctx.send(poise::CreateReply::default()
        .content(format!("`{}` = `{}`", expression, result))
    ).await?;

    Ok(())
}



