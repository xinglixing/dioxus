#![allow(non_snake_case, unused)]
use dioxus::prelude::*;
use dioxus_fullstack::prelude::*;

#[cfg(feature = "ssr")]
#[derive(Default, Clone)]
struct ServerFunctionState {
    call_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

fn main() {
    #[cfg(feature = "web")]
    dioxus_web::launch_with_props(
        app,
        // Get the root props from the document
        get_root_props_from_document().unwrap_or_default(),
        dioxus_web::Config::new().hydrate(true),
    );
    #[cfg(feature = "ssr")]
    {
        use axum::body::Body;
        use axum::extract::Path;
        use axum::extract::State;
        use axum::http::Request;
        use axum::routing::get;
        use std::sync::Arc;

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));
                axum::Server::bind(&addr)
                    .serve(
                        axum::Router::new()
                            // Serve the dist folder with the static javascript and WASM files created by the dixous CLI
                            .serve_static_assets("./dist")
                            // Register server functions
                            .register_server_fns_with_handler("", |func| {
                                move |State(server_fn_state): State<ServerFunctionState>, req: Request<Body>| async move {
                                    let (parts, body) = req.into_parts();
                                    let parts: Arc<RequestParts> = Arc::new(parts.into());
                                    let mut server_context = DioxusServerContext::new(parts.clone());
                                    server_context.insert(server_fn_state);
                                    server_fn_handler(server_context, func.clone(), parts, body).await
                                }
                            })
                            .with_state(ServerFunctionState::default())
                            // Connect to the hot reload server in debug mode
                            .connect_hot_reload()
                            // Render the application. This will serialize the root props (the intial count) into the HTML
                            .route(
                                "/",
                                get(move |State(ssr_state): State<SSRState>| async move { axum::body::Full::from(
                                    ssr_state.render(
                                        &ServeConfigBuilder::new(
                                            app,
                                            0,
                                        )
                                        .build(),
                                    )
                                )}),
                            )
                            // Render the application with a different intial count
                            .route(
                                "/:initial_count",
                                get(move |Path(intial_count): Path<usize>, State(ssr_state): State<SSRState>| async move { axum::body::Full::from(
                                    ssr_state.render(
                                        &ServeConfigBuilder::new(
                                            app,
                                            intial_count,
                                        )
                                        .build(),
                                    )
                                )}),
                            )
                            .with_state(SSRState::default())
                            .into_make_service(),
                    )
                    .await
                    .unwrap();
            });
    }
}

fn app(cx: Scope<usize>) -> Element {
    let mut count = use_state(cx, || *cx.props);

    cx.render(rsx! {
        h1 { "High-Five counter: {count}" }
        button { onclick: move |_| count += 1, "Up high!" }
        button { onclick: move |_| count -= 1, "Down low!" }
        button {
            onclick: move |_| {
                to_owned![count];
                let sc = cx.sc();
                async move {
                    // Call the server function just like a local async function
                    if let Ok(new_count) = double_server(sc, *count.current()).await {
                        count.set(new_count);
                    }
                }
            },
            "Double"
        }
    })
}

// We use the "getcbor" encoding to make caching easier
#[server(DoubleServer, "", "getcbor")]
async fn double_server(cx: DioxusServerContext, number: usize) -> Result<usize, ServerFnError> {
    // Perform some expensive computation or access a database on the server
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let result = number * 2;

    println!(
        "User Agent {:?}",
        cx.request_parts().headers.get("User-Agent")
    );

    // Set the cache control header to 1 hour on the post request
    cx.response_headers_mut()
        .insert("Cache-Control", "max-age=3600".parse().unwrap());

    // Get the server function state
    let server_fn_state = cx.get::<ServerFunctionState>().unwrap();
    let call_count = server_fn_state
        .call_count
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    println!("server functions have been called {call_count} times");

    println!("server calculated {result}");

    Ok(result)
}
