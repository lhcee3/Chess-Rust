use leptos::*;

#[component]
pub fn App(cx: Scope) -> impl IntoView {
    // Renders 8x8 grid with square names like "a8", "b8", ..., "h1"
    let board = (0..8)
        .flat_map(|row| {
            (0..8).map(move |col| {
                let color = if (row + col) % 2 == 0 { "white" } else { "gray" };
                let label = format!("{}{}", (b'a' + col as u8) as char, 8 - row);
                view! { cx,
                    <div class=format!("square {}", color)>
                        {label}
                    </div>
                }
            })
        })
        .collect_view(cx);

    view! { cx,
        <main>
            <h1>"Rust Chess with Leptos"</h1>
            <div class="chessboard">{board}</div>
        </main>
    }
}
