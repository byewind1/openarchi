# Optional local plugin examples

These original Apache-2.0 manifests declare finite, typed operations over the shared model. Read a manifest in **Plugins and jobs…**, inspect its declared capabilities, then explicitly enable it. Reading a declaration never runs an external program.

- `openarchi-quantities.json` and `example-quantities.json` use the built-in authored-quantity operation. No network or AI key is required.
- `openarchi-solar.json` declares the built-in solar study. Results are estimates with explicit inputs, not regulatory conclusions.
- `example-zone-program.json` demonstrates an external stdio provider. Its independent source is `tools/zone-program-provider` in the repository; no provider executable is bundled. Build and review it separately, then choose and authorize its executable identity in the app.

External providers run under your OS account, not inside a security sandbox. Enable only trusted programs. Commands and results are tied to the declared inputs and do not grant arbitrary model writes. Cancelled, malformed or stale results do not silently replace the model. Platform and operation support varies; see the main README.
