# TODO / open discussions

## Agent event waiting: seq-gap awareness + long-poll (requested 2026-08-31)

Requirement (Jono): an MCP agent should be able to wait for "the next
event with seq greater than N" without polling. Flow: agent queries and
learns the last seq (say 11), posts a reply, the response says its event
was 14 — so it knows it missed 12–13 and fetches them — then asks to
wait for anything after 14; the daemon holds the request until such an
event exists.

State of play (verified in the code, to discuss before building more):

- Every query already returns `seq` (`ReviewSnapshot.seq`, MCP
  `get_review`/`list_comments` results), and every mutation returns the
  committed `Event`, which carries its `seq` — the "I posted #14 while I
  had seen #11" gap detection already works.
- The long-poll already exists end to end: `ops::poll_events` subscribes
  with `Since::After { seq }` (the daemon replays the gap *before*
  answering, so the query→wait race cannot drop events), waits up to
  `timeout` for at least one event, drains what is queued, unsubscribes.
  The MCP `subscribe_events` tool exposes it (`since_seq`, `timeout_ms`,
  `max`, scope by review/workspace/`awaiting_agent`) and returns
  `last_seq` to pass back — covered by
  `crates/moor-mcp/tests/mcp.rs::subscribe_events_long_polls_and_resumes`.

Open questions for the chat:

- Is the existing `subscribe_events` long-poll sufficient, or is a
  dedicated `Request::Events { after, wait_ms }` (no subscribe dance)
  worth having for one-shot clients?
- Should mutation *responses* also return the events the writer missed
  (everything between its last-seen seq and its own event), saving the
  follow-up fetch?
- `SeqTooOld` recovery guidance for agents that were away too long
  (currently: error names the oldest replayable seq).
