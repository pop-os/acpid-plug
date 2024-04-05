use futures_util::StreamExt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let mut ac_plug_events = acpid_plug::connect().await?;

    while let Some(event) = ac_plug_events.next().await {
        match event {
            Ok(event) => println!("{:?}", event),
            Err(why) => eprintln!("error: {}", why),
        }
    }

    Ok(())
}
