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
Item {
  id: client
  visible: false
  width: 0
  height: 0

  signal statusLoaded(var report)
  signal statusFailed(string message)
  signal mutationSucceeded(string opId, string kind, var result)
  signal mutationFailed(string opId, string kind, string message)
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
  property int reconnectDelayMs: 1000
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
    if (!rpcSocket.connected) return -1
    var id = client.nextRequestId++
    var message = { "jsonrpc": "2.0", "id": id, "method": method }
    if (params !== undefined && params !== null) message.params = params
    client.requests[id] = meta
    rpcSocket.write(JSON.stringify(message) + "\n")
    rpcSocket.flush()
    return id
  }

  function connectSocket() {
    if (!client.socketPath || client.socketActive || client.socketConnecting) return
    client.socketConnecting = true
    rpcSocket.path = client.socketPath
    rpcSocket.connected = true
    handshakeTimer.restart()
  }

  function scheduleReconnect() {
    if (client.socketActive || client.socketConnecting) return
    if (reconnectTimer.running) return
    reconnectTimer.interval = client.reconnectDelayMs
    reconnectTimer.restart()
    client.reconnectDelayMs = Math.min(30000, client.reconnectDelayMs * 2)
  }

  function startHandshake() {
    client.requests = ({})
    client.activeOps = ({})
    client.lastKnownRevision = 0
    client.lastStatusRevision = 0
    client.lastAppliedRevision = 0
    client.pendingStatusRevision = 0
    client.statusInFlight = false
    handshakeTimer.restart()
    client.request("hello", { "protocol": [1], "client": "salah-widget", "actor": "widget" },
      { "type": "hello" })
  }

  function dropSocket(reason) {
    client.socketConnecting = false
    client.handshaking = false
    handshakeTimer.stop()
    if (rpcSocket.connected) rpcSocket.connected = false
    if (client.socketActive) {
      client.socketActive = false
      client.pendingStatusRevision = 0
      client.statusInFlight = false
      client.transportChanged(false)
    }
    client.failOutstanding(reason)
    client.scheduleReconnect()
  }

  function failOutstanding(reason) {
    var pending = client.requests
    client.requests = ({})
    client.activeOps = ({})
    for (var key in pending) {
      var meta = pending[key]
      if (!meta) continue
      if (meta.type === "status") {
        client.statusInFlight = false
        client.lastStatusRevision = 0
        client.statusFailed(reason)
      } else if (meta.type === "mutation") {
        client.mutationFailed(meta.opId, meta.kind, reason)
      }
    }
  }

  function handleConnectionState() {
    if (rpcSocket.connected) {
      if (client.socketActive || client.handshaking) return
      client.socketConnecting = false
      client.handshaking = true
      client.startHandshake()
      return
    }
    if (client.socketActive || client.socketConnecting || client.handshaking)
      client.dropSocket("connection closed")
  }

  function handleSocketError(error) {
    if (rpcSocket.connected) {
      rpcSocket.connected = false
      Qt.callLater(client.handleConnectionState)
    } else {
      client.handleConnectionState()
    }
  }

  function handleLine(line) {
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
      else if (meta.type === "mutation") {
        delete client.activeOps[meta.opId]
        client.mutationFailed(meta.opId, meta.kind, detail)
      } else if (meta.type === "hello" || meta.type === "subscribe") {
        client.dropSocket(detail)
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
      client.dropSocket("invalid hello response")
      return
    }
    if (Number(result.protocol) !== 1) {
      client.dropSocket("unsupported protocol")
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
    client.reconnectDelayMs = 1000
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
    // Our own mutation response already carries the canonical rows.
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
      client.mutationFailed(opId, kind, "connection closed")
    }
  }

  function fallback(opId, kind, command) {
    client.fallbackQueue = client.fallbackQueue.concat([{
      "opId": opId, "kind": kind, "command": command
    }])
    client.runNextFallback()
  }

  function runNextFallback() {
    if (actionProcess.running || client.fallbackQueue.length === 0) return
    client.activeFallback = client.fallbackQueue[0]
    client.fallbackQueue = client.fallbackQueue.slice(1)
    actionProcess.command = client.activeFallback.command
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

  function setDone(opId, itemId, done) {
    if (client.socketActive) {
      client.sendMutation(opId, "done", done ? "entry.done" : "entry.undo",
        { "item_id": String(itemId) })
      return
    }
    client.fallback(opId, "done",
      ["salah", "item", done ? "done" : "undo", "--json", String(itemId)])
  }

  function logCount(opId, itemId, delta) {
    if (client.socketActive) {
      client.sendMutation(opId, "logCount", "entry.log",
        { "item_id": String(itemId), "kind": "count", "value": Number(delta) || 0 })
      return
    }
    client.fallback(opId, "logCount",
      ["salah", "item", "log", "--json", String(itemId), String(Number(delta) || 0)])
  }

  function logMinutes(opId, itemId, delta) {
    if (client.socketActive) {
      client.sendMutation(opId, "logMinutes", "entry.log",
        { "item_id": String(itemId), "kind": "minutes", "value": Number(delta) || 0 })
      return
    }
    client.fallback(opId, "logMinutes",
      ["salah", "item", "log", "--json", "--minutes", String(itemId), String(Number(delta) || 0)])
  }

  function startTimer(opId, itemId) {
    if (client.socketActive) {
      client.sendMutation(opId, "startTimer", "timer.start", { "item_id": String(itemId) })
      return
    }
    client.fallback(opId, "startTimer", ["salah", "item", "start", "--json", String(itemId)])
  }

  function stopTimer(opId, itemId) {
    if (client.socketActive) {
      client.sendMutation(opId, "stopTimer", "timer.stop", { "item_id": String(itemId) })
      return
    }
    client.fallback(opId, "stopTimer", ["salah", "item", "stop", "--json", String(itemId)])
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

  Component.onCompleted: {
    initialStatusTimer.start()
    client.connectSocket()
  }

  Timer {
    id: initialStatusTimer
    interval: 500
    repeat: false
    onTriggered: if (!client.socketActive) client.refreshStatus()
  }

  Timer {
    id: reconnectTimer
    repeat: false
    onTriggered: client.connectSocket()
  }

  Timer {
    id: handshakeTimer
    interval: 5000
    repeat: false
    onTriggered: {
      if (!client.socketActive) {
        rpcSocket.connected = false
        client.dropSocket("handshake timed out")
      }
    }
  }

  Socket {
    id: rpcSocket
    parser: SplitParser {
      splitMarker: "\n"
      onRead: function(line) { client.handleLine(line) }
    }
    onConnectionStateChanged: client.handleConnectionState()
    onError: function(error) { client.handleSocketError(error) }
  }

  Process {
    id: statusProcess
    command: ["salah", "status", "--json"]
    stdout: StdioCollector { id: statusStdout; waitForEnd: true }
    stderr: StdioCollector { id: statusStderr; waitForEnd: true }
    onExited: function(exitCode) {
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
    stdout: StdioCollector { id: actionStdout; waitForEnd: true }
    stderr: StdioCollector { id: actionStderr; waitForEnd: true }
    onExited: function(exitCode) {
      var active = client.activeFallback
      client.activeFallback = null
      if (active) {
        if (exitCode === 0) {
          client.mutationSucceeded(active.opId, active.kind, {
            "fallback": true, "raw": String(actionStdout.text || "")
          })
        } else {
          client.mutationFailed(active.opId, active.kind,
            String(actionStderr.text || "").trim().replace(/[\r\n]+/g, " "))
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
