const HTML: &str = r#"
            <!doctype html>
            <html lang="en">
            <head>
            <title>api.fabianlars.de</title>
            <link href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@3/swagger-ui.css" rel="stylesheet">
            </head>
            <body>
                <div id="swagger-ui"></div>
                <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@3/swagger-ui-bundle.js" charset="UTF-8"> </script>
                <script>
                    window.onload = function() {
                    const ui = SwaggerUIBundle({
                        "dom_id": "\#swagger-ui",
                        presets: [
                        SwaggerUIBundle.presets.apis,
                        SwaggerUIBundle.SwaggerUIStandalonePreset
                        ],
                        layout: "BaseLayout",
                        deepLinking: true,
                        showExtensions: true,
                        showCommonExtensions: true,
                        url: "/openapi.json",
                    })
                    window.ui = ui;
                };
            </script>
            </body>
            </html>
        "#;

pub(crate) fn openapi_docs(
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let json = warp::path("openapi.json").and(warp::fs::file("./openapi.json"));
    let docs = warp::path("docs").map(move || warp::reply::html(HTML));
    let docs_root = warp::path::end().map(move || warp::reply::html(HTML));
    json.or(docs).or(docs_root)
}
