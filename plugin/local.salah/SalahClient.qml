import QtQuick
import Quickshell
import Quickshell.Io

// Transport for the widget: JSON-RPC 2.0 over the daemon's NDJSON Unix
// socket, with the `salah` CLI as a fallback when the daemon is unavailable.
//
// The socket path is the protocol-mandated `$XDG_RUNTIME_DIR/salah/salah.sock`.
// Requests are correlated by JSON-RPC id; every mutation gets a fresh op_id
// from `newOpId()` so a retry can reuse it safely. The CLI fallback keeps the
// widget usable without a running daemon and is retried on a backoff timer.
//
// The one Socket below is a direct, static child: it is never created in a
// Loader/Repeater/Component and never destroyed to reconnect. Mutating
// `connected`/`path` from inside a Socket signal handler makes Quickshell
// delete the QLocalSocket while QAbstractSocketPrivate::setErrorAndEmit() is
// still on the stack (a shell segfault). All socket reconfiguration therefore
// happens only in attemptConnect(), reached from onCompleted or reconnectTimer.
Item {
  id: client
  visible: false
  width: 0
  height: 0

  signal statusLoaded(var report)
  signal statusFailed(string message)
  signal projectsLoaded(var projects)
  signal metricsLoaded(var metrics)
  // Entity scopes named by a `changed` notification (items, sessions,
  // projects, tags, metrics, marks), for lists the status does not carry.
  signal entitiesChanged(var scopes)
  signal mutationSucceeded(string opId, string kind, var result)
  signal mutationFailed(string opId, string kind, string message, var data)
  signal transportChanged(bool socketActive)

  readonly property string socketPath: {
    var dir = Quickshell.env("XDG_RUNTIME_DIR")
    if (dir === undefined || dir === null || String(dir) === "") return ""
    return String(dir) + "/salah/salah.sock"
  }
  property bool socketActive: false
  property string serverName: ""
  property int lastKnownRevision: 0
  property int lastStatusRevision: 0
  property int lastAppliedRevision: 0
  property int requestedStatusRevision: 0
  property int pendingStatusRevision: 0
  property bool statusInFlight: false
  property int nextRequestId: 1
  property var requests: ({})
  property var activeOps: ({})
  property bool socketConnecting: false
  property bool handshaking: false
  property bool alive: true
  property int reconnectDelayMs: 500
  property var fallbackQueue: []
  property var activeFallback: null
  property bool statusAfterFallback: false
  property bool fallbackStatusPending: false

  function newOpId() {
    function hex(count) {
      var digits = "0123456789abcdef"
      var out = ""
      for (var i = 0; i < count; i++) out += digits.charAt(Math.floor(Math.random() * 16))
      return out
    }
    return hex(8) + "-" + hex(4) + "-4" + hex(3) + "-"
      + "89ab".charAt(Math.floor(Math.random() * 4)) + hex(3) + "-" + hex(12)
  }

  function request(method, params, meta) {
    if (!client.alive || !rpcSocket.connected) return -1
    var id = client.nextRequestId++
    var message = { "jsonrpc": "2.0", "id": id, "method": method }
    if (params !== undefined && params !== null) message.params = params
    client.requests[id] = meta
    rpcSocket.write(JSON.stringify(message) + "\n")
    rpcSocket.flush()
    return id
  }

  // Safe point for every socket mutation. Only onCompleted (after the item is
  // fully built) and reconnectTimer.onTriggered call this, never a Socket
  // signal handler, so closing a stale QLocalSocket cannot delete it while Qt
  // is still emitting from it.
  function attemptConnect() {
    if (!client.alive) return
    if (client.socketActive || client.socketConnecting) return
    if (rpcSocket.connected) rpcSocket.connected = false
    if (!client.socketPath) {
      client.scheduleReconnect()
      return
    }
    if (rpcSocket.path !== client.socketPath) rpcSocket.path = client.socketPath
    client.socketConnecting = true
    rpcSocket.connected = true
  }

  function scheduleReconnect() {
    if (!client.alive) return
    if (client.socketActive || client.socketConnecting) return
    if (reconnectTimer.running) return
    reconnectTimer.interval = client.reconnectDelayMs
    reconnectTimer.restart()
    client.reconnectDelayMs = Math.min(30000, client.reconnectDelayMs * 2)
  }

  function startHandshake() {
    if (!client.alive || !rpcSocket.connected) return
    client.requests = ({})
    client.activeOps = ({})
    client.lastKnownRevision = 0
    client.lastStatusRevision = 0
    client.lastAppliedRevision = 0
    client.pendingStatusRevision = 0
    client.statusInFlight = false
    handshakeTimer.restart()
    if (client.request("hello", { "protocol": [1], "client": "salah-widget", "actor": "widget" },
      { "type": "hello" }) < 0) {
      client.markDisconnected("connection closed")
    }
  }

  // Records a broken connection. Deliberately performs no socket mutation:
  // callers include Socket.onError/onConnectionStateChanged and SplitParser
  // onRead, all of which run inside Qt's QLocalSocket emission. The reconnect
  // Timer does the actual close/reopen on a safe stack.
  function markDisconnected(reason) {
    if (!client.alive) return
    client.socketConnecting = false
    client.handshaking = false
    handshakeTimer.stop()
    var wasActive = client.socketActive
    client.socketActive = false
    client.pendingStatusRevision = 0
    client.statusInFlight = false
    if (wasActive) client.transportChanged(false)
    client.failOutstanding(reason)
    client.scheduleReconnect()
  }

  function failOutstanding(reason) {
    var pending = client.requests
    client.requests = ({})
    client.activeOps = ({})
    var failures = []
    for (var key in pending) {
      var meta = pending[key]
      if (!meta) continue
      if (meta.type === "status") {
        client.statusInFlight = false
        client.lastStatusRevision = 0
        failures.push({ "type": "status", "reason": reason })
      } else if (meta.type === "mutation") {
        failures.push({ "type": "mutation", "opId": meta.opId,
          "kind": meta.kind, "reason": reason })
      }
    }
    if (failures.length === 0) return
    // Fail asynchronously: these handlers may call back into the widget, and
    // the disconnect/error path must unwind before anyone can destroy it.
    Qt.callLater(function() {
      if (!client.alive) return
      for (var i = 0; i < failures.length; i++) {
        var failure = failures[i]
        if (failure.type === "status") client.statusFailed(failure.reason)
        else client.mutationFailed(failure.opId, failure.kind, failure.reason, {})
      }
    })
  }

  function handleConnectionState() {
    if (!client.alive) return
    if (rpcSocket.connected) {
      if (client.socketActive || client.handshaking) return
      client.socketConnecting = false
      client.handshaking = true
      handshakeTimer.restart()
      // A write is safe here; unlike connected/path it does not recreate the
      // underlying QLocalSocket. Defer it anyway to leave the signal first.
      Qt.callLater(client.startHandshake)
      return
    }
    if (client.socketActive || client.socketConnecting || client.handshaking)
      client.markDisconnected("connection closed")
  }

  function handleSocketError(error) {
    // Only record state and arm the Timer. Quickshell's Socket::onSocketError
    // still owns the emitting QLocalSocket; setting `connected` here calls
    // disconnectFromServer() and deleteLater() from inside that emission,
    // which is the reported segfault.
    client.markDisconnected("socket error")
  }

  function handleLine(line) {
    if (!client.alive) return
    var text = String(line || "").trim()
    if (text === "") return
    var message
    try {
      message = JSON.parse(text)
    } catch (error) {
      return
    }
    if (!message || typeof message !== "object") return
    var method = String(message.method || "")
    if (method === "changed") {
      client.handleChanged(message.params || {})
      return
    }
    if (method === "clock.boundary") {
      // The core pushes the authoritative boundary; refetch its status
      // projection instead of running a local boundary timer.
      client.refreshStatus()
      return
    }
    if (message.id === undefined || message.id === null) return
    var meta = client.requests[message.id]
    if (!meta) return
    delete client.requests[message.id]

    if (message.error) {
      var detail = client.errorDetail(message.error)
      if (meta.type === "status") client.handleStatusFailure(detail)
      else if (meta.type === "projects" || meta.type === "metrics") {
        // The planner still works without these lists; the panels say so.
      }
      else if (meta.type === "mutation") {
        delete client.activeOps[meta.opId]
        client.mutationFailed(meta.opId, meta.kind, detail, message.error.data || {})
      } else if (meta.type === "hello" || meta.type === "subscribe") {
        client.markDisconnected(detail)
      }
      return
    }

    var result = message.result
    if (meta.type === "hello") {
      client.handleHello(result)
    } else if (meta.type === "subscribe") {
      client.handleSubscribed(result)
    } else if (meta.type === "status") {
      client.handleStatusResult(meta, result)
    } else if (meta.type === "projects") {
      client.projectsLoaded(result || [])
    } else if (meta.type === "metrics") {
      client.metricsLoaded(result || [])
    } else if (meta.type === "mutation") {
      delete client.activeOps[meta.opId]
      var revision = Number(result && result.revision) || 0
      client.lastKnownRevision = Math.max(client.lastKnownRevision, revision)
      // The response already carries the canonical rows for this revision.
      client.lastAppliedRevision = Math.max(client.lastAppliedRevision, revision)
      client.lastStatusRevision = Math.max(client.lastStatusRevision, revision)
      client.mutationSucceeded(meta.opId, meta.kind, result)
    }
  }

  function errorDetail(error) {
    if (!error) return ""
    var data = error.data || {}
    var kind = String(data.kind || "")
    var message = String(error.message || "")
    if (message !== "") return message
    return kind
  }

  function handleHello(result) {
    if (!result) {
      client.markDisconnected("invalid hello response")
      return
    }
    if (Number(result.protocol) !== 1) {
      client.markDisconnected("unsupported protocol")
      return
    }
    handshakeTimer.stop()
    client.serverName = String(result.server || "")
    client.lastKnownRevision = Number(result.revision) || 0
    client.request("subscribe", { "topics": ["changes", "clock"] },
      { "type": "subscribe" })
  }

  function handleSubscribed(result) {
    client.socketActive = true
    client.socketConnecting = false
    client.handshaking = false
    client.reconnectDelayMs = 500
    handshakeTimer.stop()
    client.lastKnownRevision = Math.max(client.lastKnownRevision,
      Number(result && result.revision) || 0)
    client.transportChanged(true)
    client.requestStatus(client.lastKnownRevision)
  }

  function requestStatus(revision) {
    if (!client.socketActive) return
    var target = Number(revision)
    if (!isFinite(target) || target < 0) target = client.lastKnownRevision
    if (client.statusInFlight) {
      client.pendingStatusRevision = Math.max(client.pendingStatusRevision, target)
      return
    }
    client.statusInFlight = true
    client.requestedStatusRevision = target
    if (client.request("status.get", {}, { "type": "status", "revision": target }) < 0) {
      client.statusInFlight = false
      client.statusFailed("connection closed")
    }
  }

  function handleStatusResult(meta, report) {
    client.statusInFlight = false
    var metaRevision = Number(meta.revision) || 0
    client.lastStatusRevision = Math.max(client.lastStatusRevision, metaRevision)
    // A mutation response applied after this snapshot was requested is newer
    // than the snapshot; refetch instead of overwriting canonical rows.
    if (metaRevision < client.lastAppliedRevision) {
      client.pendingStatusRevision = 0
      client.requestStatus(client.lastAppliedRevision)
      return
    }
    client.statusLoaded(report)
    if (client.pendingStatusRevision > metaRevision) {
      var next = client.pendingStatusRevision
      client.pendingStatusRevision = 0
      client.requestStatus(next)
    } else {
      client.pendingStatusRevision = 0
    }
  }

  function handleStatusFailure(message) {
    client.statusInFlight = false
    client.lastStatusRevision = 0
    client.statusFailed(message)
  }

  function handleChanged(params) {
    var revision = Number(params.revision) || 0
    client.lastKnownRevision = Math.max(client.lastKnownRevision, revision)
    var opId = String(params.op_id || "")
    client.entitiesChanged(params.scopes instanceof Array ? params.scopes : [])
    // Our own mutation response already carries the canonical rows, and the
    // widget refetches status after every mutation it sends.
    if (opId !== "" && client.activeOps[opId]) return
    if (revision !== 0 && revision <= client.lastStatusRevision) return
    client.requestStatus(revision)
  }

  // ---- mutations -------------------------------------------------------

  function sendMutation(opId, kind, method, params) {
    params = params || {}
    params.op_id = opId
    client.activeOps[opId] = true
    if (client.request(method, params, { "type": "mutation", "opId": opId, "kind": kind }) < 0) {
      delete client.activeOps[opId]
      client.mutationFailed(opId, kind, "connection closed", {})
    }
  }

  function fallback(opId, kind, command, rpcInput) {
    client.fallbackQueue = client.fallbackQueue.concat([{
      "opId": opId, "kind": kind, "command": command, "rpcInput": rpcInput || ""
    }])
    client.runNextFallback()
  }

  function runNextFallback() {
    if (actionProcess.running || client.fallbackQueue.length === 0) return
    client.activeFallback = client.fallbackQueue[0]
    client.fallbackQueue = client.fallbackQueue.slice(1)
    actionProcess.command = client.activeFallback.command
    actionProcess.stdinEnabled = client.activeFallback.rpcInput !== ""
    actionProcess.running = true
  }

  function addItem(opId, title, block) {
    if (client.socketActive) {
      var params = { "quick_text": String(title || "") }
      if (String(block || "") !== "") params.block = String(block)
      client.sendMutation(opId, "add", "item.add", params)
      return
    }
    var command = ["salah", "item", "add", "--json", "--quick"]
    if (String(block || "") !== "") command = command.concat(["--block", String(block)])
    command.push("--")
    command.push(String(title || ""))
    client.fallback(opId, "add", command)
  }

  function valueArgs(values) {
    var args = []
    for (var key in values || {})
      args = args.concat(["--value", String(key) + "=" + String(Number(values[key]))])
    return args
  }

  function logSession(opId, itemId, projectId, minutes, values, note, block) {
    if (client.socketActive) {
      var params = { "values": values || {} }
      if (String(itemId || "") !== "") params.item_id = String(itemId)
      if (String(projectId || "") !== "") params.project_id = String(projectId)
      if (minutes !== undefined && minutes !== null) params.minutes = Number(minutes)
      if (String(note || "") !== "") params.note = String(note)
      if (String(block || "") !== "") params.block = String(block)
      client.sendMutation(opId, "logSession", "session.log", params)
      return
    }
    var command = ["salah", "session", "log", "--json"]
    if (String(itemId || "") !== "") command = command.concat(["--item", String(itemId)])
    if (String(projectId || "") !== "") command = command.concat(["--project", String(projectId)])
    if (minutes !== undefined && minutes !== null) command = command.concat(["--minutes", String(Number(minutes))])
    if (String(block || "") !== "") command = command.concat(["--block", String(block)])
    command = command.concat(client.valueArgs(values))
    if (String(note || "") !== "") command.push("--note=" + String(note))
    client.fallback(opId, "logSession", command)
  }

  function startSession(opId, itemId, projectId) {
    if (client.socketActive) {
      var params = {}
      if (String(itemId || "") !== "") params.item_id = String(itemId)
      if (String(projectId || "") !== "") params.project_id = String(projectId)
      client.sendMutation(opId, "startSession", "session.start", params)
      return
    }
    var command = ["salah", "session", "start", "--json"]
    if (String(itemId || "") !== "") command = command.concat(["--item", String(itemId)])
    else if (String(projectId || "") !== "") command = command.concat(["--project", String(projectId)])
    client.fallback(opId, "startSession", command)
  }

  function stopSession(opId, params) {
    if (client.socketActive) {
      client.sendMutation(opId, "stopSession", "session.stop", params)
      return
    }
    var command = ["salah", "session", "stop", "--json"]
    if (params.id) command.push("--id", params.id)
    if (params.done) command.push("--done")
    if (params.minutes !== undefined) command.push("--minutes", String(params.minutes))
    command = command.concat(client.valueArgs(params.values))
    if (params.note) command.push("--note=" + params.note)
    client.fallback(opId, "stopSession", command)
  }

  function runningMutation(opId, method, params) {
    if (client.socketActive) {
      client.sendMutation(opId, method, method, params)
      return
    }
    // The in-process CLI dispatcher accepts the same bump/note protocol.
    params.op_id = opId
    var hello = { "jsonrpc": "2.0", "id": 1, "method": "hello",
      "params": { "protocol": [1], "client": "salah-widget", "actor": "widget" } }
    var request = { "jsonrpc": "2.0", "id": 2, "method": method, "params": params }
    client.fallback(opId, method, ["salah", "rpc"],
      JSON.stringify(hello) + "\n" + JSON.stringify(request) + "\n")
  }

  function removeSession(opId, kind, sessionId) {
    if (client.socketActive) {
      client.sendMutation(opId, kind, "session.remove", { "id": String(sessionId) })
      return
    }
    client.fallback(opId, kind, ["salah", "session", "remove", "--json", String(sessionId)])
  }

  function editSessionMinutes(opId, sessionId, minutes) {
    if (client.socketActive) {
      client.sendMutation(opId, "editSession", "session.edit",
        { "id": String(sessionId), "minutes": Number(minutes) })
      return
    }
    client.fallback(opId, "editSession", ["salah", "session", "edit", "--json",
      "--minutes", String(Number(minutes)), String(sessionId)])
  }

  function moveItem(opId, itemId, block, index, afterId, beforeId) {
    if (client.socketActive) {
      var params = { "id": String(itemId), "block": String(block) }
      // A row already exposes its neighbours, so RPC sends a stable anchor
      // rather than a numeric index that may have gone stale.
      if (String(afterId || "") !== "") params.after_id = String(afterId)
      else if (String(beforeId || "") !== "") params.before_id = String(beforeId)
      else params.index = Number(index) || 0
      client.sendMutation(opId, "move", "item.move", params)
      return
    }
    client.fallback(opId, "move", ["salah", "item", "move", "--json", String(itemId),
      "--block", String(block), "--index", String(Number(index) || 0)])
  }

  function editItem(opId, itemId, title) {
    if (client.socketActive) {
      client.sendMutation(opId, "edit", "item.edit",
        { "id": String(itemId), "title": String(title) })
      return
    }
    client.fallback(opId, "edit",
      ["salah", "item", "edit", "--json", String(itemId), "--title=" + String(title)])
  }

  function removeItem(opId, itemId) {
    if (client.socketActive) {
      client.sendMutation(opId, "remove", "item.remove", { "id": String(itemId) })
      return
    }
    client.fallback(opId, "remove", ["salah", "item", "remove", "--json", String(itemId)])
  }

  function markPrayer(opId, prayer, marked) {
    if (client.socketActive) {
      client.sendMutation(opId, "prayer", marked ? "prayer.mark" : "prayer.unmark",
        { "prayer": String(prayer) })
      return
    }
    client.fallback(opId, "prayer", ["salah", "prayer", marked ? "mark" : "unmark",
      "--json", String(prayer)])
  }

  // ---- status refresh --------------------------------------------------

  function refreshStatus() {
    if (client.socketActive) {
      client.requestStatus(client.lastKnownRevision)
      return
    }
    if (actionProcess.running || client.fallbackQueue.length > 0) {
      client.statusAfterFallback = true
      return
    }
    if (statusProcess.running) {
      client.fallbackStatusPending = true
      return
    }
    statusProcess.running = true
  }

  function loadProjects() {
    if (client.socketActive)
      client.request("projects.list", { "archived": false }, { "type": "projects" })
  }

  function loadMetrics() {
    if (client.socketActive)
      client.request("metrics.list", { "archived": false }, { "type": "metrics" })
  }

  Component.onCompleted: {
    initialStatusTimer.start()
    client.attemptConnect()
  }

  Component.onDestruction: {
    // Quickshell's own examples never touch Socket.connected while tearing a
    // Socket owner down; the QLocalSocket is owned by the Socket and is
    // destroyed with it. Mutating it here would re-enter the socket signal
    // path during destruction. Stop the Timers and stop accepting callbacks.
    client.alive = false
    reconnectTimer.stop()
    handshakeTimer.stop()
    initialStatusTimer.stop()
  }

  Timer {
    id: initialStatusTimer
    interval: 500
    repeat: false
    onTriggered: if (client.alive && !client.socketActive) client.refreshStatus()
  }

  Timer {
    id: reconnectTimer
    repeat: false
    // The only recurring place that toggles Socket.connected/path. A Timer
    // callback runs outside Qt's QLocalSocket emission, so closing a stale
    // socket here is safe.
    onTriggered: client.attemptConnect()
  }

  Timer {
    id: handshakeTimer
    interval: 5000
    repeat: false
    onTriggered: {
      if (client.alive && !client.socketActive) client.markDisconnected("handshake timed out")
    }
  }

  // One long-lived Socket for the widget's whole lifetime: static, directly
  // owned, never in a Loader/Repeater/Component. Its handlers only record
  // state and arm reconnectTimer; they never call back into the Socket.
  Socket {
    id: rpcSocket
    parser: SplitParser {
      splitMarker: "\n"
      onRead: function(line) {
        if (client.alive) client.handleLine(line)
      }
    }
    onConnectionStateChanged: if (client.alive) client.handleConnectionState()
    onError: function(error) { if (client.alive) client.handleSocketError(error) }
  }

  Process {
    id: statusProcess
    command: ["salah", "status", "--json"]
    stdout: StdioCollector { id: statusStdout; waitForEnd: true }
    stderr: StdioCollector { id: statusStderr; waitForEnd: true }
    onExited: function(exitCode) {
      if (!client.alive) return
      // A socket snapshot supersedes one that was already in flight when the
      // connection came up.
      if (!client.socketActive) {
        if (exitCode === 0) {
          try {
            client.statusLoaded(JSON.parse(String(statusStdout.text || "").trim()))
          } catch (error) {
            client.statusFailed(String(error))
          }
        } else {
          client.statusFailed(String(statusStderr.text || ""))
        }
      }
      if (client.fallbackStatusPending) {
        client.fallbackStatusPending = false
        client.refreshStatus()
      }
    }
  }

  Process {
    id: actionProcess
    onStarted: {
      if (client.activeFallback && client.activeFallback.rpcInput) {
        write(client.activeFallback.rpcInput)
        stdinEnabled = false
      }
    }
    stdout: StdioCollector { id: actionStdout; waitForEnd: true }
    stderr: StdioCollector { id: actionStderr; waitForEnd: true }
    onExited: function(exitCode) {
      if (!client.alive) return
      var active = client.activeFallback
      client.activeFallback = null
      if (active) {
        var failure = String(actionStderr.text || "").trim().replace(/[\r\n]+/g, " ")
        if (exitCode === 0 && active.rpcInput) {
          try {
            var lines = String(actionStdout.text || "").trim().split("\n")
            var reply = JSON.parse(lines[lines.length - 1])
            if (reply.id !== 2 || !reply.result) {
              exitCode = 1
              failure = client.errorDetail(reply.error)
            }
          } catch (error) {
            exitCode = 1
            failure = String(error)
          }
        }
        if (exitCode === 0) {
          client.mutationSucceeded(active.opId, active.kind, {
            "fallback": true, "raw": String(actionStdout.text || "")
          })
        } else {
          client.mutationFailed(active.opId, active.kind, failure, {})
        }
        client.statusAfterFallback = true
      }
      client.runNextFallback()
      if (!actionProcess.running && client.fallbackQueue.length === 0
          && client.statusAfterFallback) {
        client.statusAfterFallback = false
        client.refreshStatus()
      }
    }
  }
}
