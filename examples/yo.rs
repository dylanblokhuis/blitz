use dioxus::prelude::*;

#[tokio::main]
async fn main() {
    blitz::launch(app).await;
}

fn app(cx: Scope) -> Element {
    let mut count = use_state(cx, || 0);

    cx.render(rsx! {
        div {
          background_color: "#ff0000",
          display: "flex",
          justify_content: "center",
          align_items: "center",
          border_width: "0",
          margin: "0px",

          button {
            width: "50px",
            height: "50px",
            margin: "15px",
            background_color: "#00ff00",
            border_width: "{count}",
            border_color: "#0000ff",
            border_radius: "{count}",
            onclick: move |_| count += 2
        }
        }
        div {
          background_color: "blue",
          display: "flex",
          justify_content: "center",
          align_items: "center",
          padding: "15px",
          border_width: "0",
          margin: "0px",
        }
    })
}
