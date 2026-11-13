# Paipan chat workflows

[简体中文](README.md)

Bazi and Ziwei use separate commands, chat events, and workflow executions. Each command returns one complete report.

```text
/paipan --brith=2026-11-11 --time=13:09:00 --gender=男 --country=中国 --province=广东 --city=湛江市
/ziwei --brith=2026-11-11 --time=13:09:00 --gender=男 --country=中国 --province=广东 --city=湛江市
```

All six parameters are required. Keep the `brith` spelling; use 男/女, 中国, and separate Chinese province/city names. True solar time and UTC+08:00 are defaults. Region lookup supplies city coordinates. Reference dates and times come from execution time.

## Generate drafts

From the repository root:

```bash
node docs/architecture/event-center/examples/paipan-chat/build-workflow.mjs tmp/paipan-bazi.json YOUR_WECHAT_INSTANCE_UUID bazi
node docs/architecture/event-center/examples/paipan-chat/build-workflow.mjs tmp/paipan-ziwei.json YOUR_WECHAT_INSTANCE_UUID ziwei
```

The last argument defaults to `bazi`. Archives contain no credentials and do not publish workflows, register commands, or send messages.

```text
/paipan → Bazi event → region/birth/lunar date/true solar time → bazi/calculate → Bazi report → Notification
/ziwei  → Ziwei event → region/birth/lunar date/true solar time → ziwei/calculate → Ziwei report → Notification
```

Each workflow validates, calculates, handles errors, and sends its own report. Neither chains into the other chart. Bazi includes pillars, hidden stems, ten gods, elements, major periods, and annual periods. Ziwei includes basic information, all twelve palaces, stars, transformations, major/minor periods, and current horoscope periods. Its age display uses its own birth/reference dates, without requesting a Bazi chart.

## Configure

1. Import both event workflows and create a distinct chat-command event for each chart.
2. In the channel's Chat commands table, configure `user/paipan` with name `paipan` and `user/ziwei` with name `ziwei`, each targeting its own event. Keep the four Host-owned mappings.
3. Each workflow subscribes to its own event's `published` phase; map `arguments/input_id/conversation_id/instance_id` from matching `/payload/` fields.
4. Set `CalendarApiOrigin` and `CalendarApiAuthorization` on each app, using a dedicated API key with restricted calendar permissions. Archives do not provision keys or store root cookies. Existing application/debug-data permissions govern credential visibility.
5. Inspect calculations and reports, publish, and enable each command. A real Notification sends a message. Replace it with a text-preview Code node for non-sending draft validation and label that evidence accordingly.

## Channel history and receipts

Messages pass through the corresponding channel plugin. Channels supporting standard send receipts declare `receive.supports_send_receipts`. Host supplies request identity, SDK validates correlation, and the plugin reports the actual external recipient/conversation and available provider/client identifiers. Host owns durable history. Existing AI/command reply reservations are reused; proactive sends do not fabricate inbound inputs.

Event workflow reply destinations are recovered from the original durable command, not authorized by editable workflow parameters. A source-channel mismatch stops execution. This private-channel example does not authorize arbitrary group or broadcast destinations.

Event acceptance, workflow completion, and provider acceptance are separate facts. Acceptance does not mean read confirmation; unknown outcomes are not automatically resent. Successful admission is silent. Optional native typing is ephemeral activity, not a chat message.

AI retrieves history using the scoped, paginated channel `history_messages` tool instead of automatically replaying the whole conversation. Old send logs without reliable conversation association are not guessed into history.

The terminal send log, validated receipt, and history message commit in one transaction. An owned finalizer retains a validated receipt across caller cancellation and retries only local archival after transient storage failures. History shows an unfinished send as unknown when its input/reply owner has recovered to an uncertain state or its workflow has terminated without a durable receipt. A process crash before that transaction commits can still leave the provider outcome unconfirmed; it never authorizes a resend.

## Mock draft

`build-mock.mjs` is an earlier fixed example for six-parameter validation, with `mock:true,sent:false`. It does not prove real event delivery, calculation, or WeChat receipt. Validate the separate workflows using an explicitly isolated simulated channel; the former combined two-report command is not the expected behavior.
