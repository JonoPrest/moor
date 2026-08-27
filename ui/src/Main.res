// Entry point: mounts <App/> and pulls in the stylesheet (Tailwind v4 via
// the Vite plugin; see styles/app.css for the tokens and semantic classes).

%%raw(`import "./styles/app.css"`)

switch ReactDOM.querySelector("#root") {
| Some(root) => ReactDOM.Client.createRoot(root)->ReactDOM.Client.Root.render(<App />)
| None => Console.error("moor: #root not found")
}
