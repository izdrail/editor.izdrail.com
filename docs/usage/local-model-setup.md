# Local model quick setup

Open **Agent: Open AI Settings** from the command palette, or use the Agent panel's gear/connection summary. The shared connection card offers explicit setup choices:

- **On-device CPU** selects Gravity's built-in local backend. It keeps the remote endpoint/model for later. It does not download a model.
- **Ollama on this computer** fills `http://127.0.0.1:11434` as the pending endpoint. Start your own Ollama server and enter an installed model's exact ID.
- **LM Studio on this computer** fills `http://127.0.0.1:1234`. Start the LM Studio API server, load a model and enter its exact model ID.

The last two choices use the existing compatible HTTP transport, not an OpenAI account or hosted service. They clear the pending model and unsaved key entry so a previous provider's model/key is not silently reused. Saved keys remain endpoint-bound and are not deleted. A `LAPCE_LLM_API_KEY` environment override still wins over saved keys, so remove that override when using a key-less server if you do not want it sent.

These are pending form changes. **Apply shared connection to Chat and ghost text** saves them. Selecting a setup choice does not apply it, test it, discover models, download anything or start a server. You may edit the endpoint for a self-hosted server before Apply. Check ghost model/automatic-trigger settings and the disclosure on the form before enabling endpoint-backed inference.

"This computer" means the machine running Gravity, not the host of an SSH project. A loopback endpoint points at that machine. For an external ACP agent, its own process/configuration still manages its model and sign-in; the shared connection does not configure it.

After applying, use **Test connection** explicitly. Successful configuration is not proof that a model is loaded or a server is reachable. The Agent model selector's model-list request is also on demand, not a startup probe.

Inspired by Zed's [local model setup](https://zed.dev/docs/ai/use-a-local-model), adapted to this editor's existing shared connection. No Zed source copied.
