package wit.worlds;

import dev.ipel.gamedev.game.PlayerEvent;
import dev.ipel.gamedev.game.SpectatorEvent;
import dev.ipel.gamedev.tictactoe.Config;
import dev.ipel.gamedev.tictactoe.GameOutcome;
import dev.ipel.gamedev.tictactoe.MoveEvent;
import dev.ipel.gamedev.tictactoe.Player;
import dev.ipel.gamedev.tictactoe.PlayerOutcome;
import dev.ipel.gamedev.tictactoe.Position;
import dev.ipel.gamedev.tictactoe.State;
import dev.ipel.gamedev.tictactoe.TttPlayerState;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/** Minimal JSON codec for TeaVM (no Jackson — breaks TeaVM export preservation). */
final class TeaVmJson {

    private TeaVmJson() {}

    static byte[] serialize(Object value) {
        return toJson(value).getBytes(StandardCharsets.UTF_8);
    }

    @SuppressWarnings("unchecked")
    static <T> T deserialize(Class<T> type, byte[] data) {
        String json = new String(data, StandardCharsets.UTF_8).trim();
        if (json.isEmpty() || json.equals("null")) {
            if (type == Config.class) {
                return (T) new Config();
            }
            throw new IllegalArgumentException("unexpected null for " + type.getName());
        }
        Object parsed = parseValue(json, 0).value;
        if (type == Config.class) {
            return (T) toConfig((JsonObject) parsed);
        }
        if (type == State.class) {
            return (T) toState((JsonObject) parsed);
        }
        if (type == Position.class) {
            return (T) toPosition((JsonArray) parsed);
        }
        if (type == Player.class) {
            return (T) Player.fromWire(((JsonString) parsed).value);
        }
        if (type == TttPlayerState.class) {
            return (T) toTttPlayerState((JsonObject) parsed);
        }
        if (type == FullStateWire.class) {
            return (T) toFullStateWire((JsonObject) parsed);
        }
        throw new IllegalArgumentException("unsupported type: " + type.getName());
    }

    static byte[] playerEvent(PlayerEvent<MoveEvent, PlayerOutcome> pe) {
        if (pe instanceof PlayerEvent.Visible<MoveEvent, PlayerOutcome> v) {
            return serializeTagged("Event", v.event());
        }
        if (pe instanceof PlayerEvent.Terminal<MoveEvent, PlayerOutcome> t) {
            return ("{\"GameOver\":\"" + t.result().toWire() + "\"}")
                    .getBytes(StandardCharsets.UTF_8);
        }
        throw new IllegalArgumentException("unknown player event");
    }

    static byte[] spectatorEvent(SpectatorEvent<MoveEvent, GameOutcome> se) {
        if (se instanceof SpectatorEvent.Visible<MoveEvent, GameOutcome> v) {
            return serializeTagged("Event", v.event());
        }
        if (se instanceof SpectatorEvent.Terminal<MoveEvent, GameOutcome> t) {
            return ("{\"GameOver\":" + gameOutcomeJson(t.result()) + "}")
                    .getBytes(StandardCharsets.UTF_8);
        }
        throw new IllegalArgumentException("unknown spectator event");
    }

    private static byte[] serializeTagged(String tag, Object payload) {
        return ("{\"" + tag + "\":" + toJson(payload) + "}").getBytes(StandardCharsets.UTF_8);
    }

    private static String gameOutcomeJson(GameOutcome outcome) {
        if (outcome instanceof GameOutcome.Win w) {
            return "{\"Win\":\"" + w.winner().toWire() + "\"}";
        }
        if (outcome instanceof GameOutcome.Draw) {
            return "{\"Draw\":null}";
        }
        throw new IllegalArgumentException("unknown game outcome");
    }

    private static String toJson(Object value) {
        if (value == null) {
            return "null";
        }
        if (value instanceof Config c) {
            return "{\"side_length\":" + c.sideLength + ",\"win_length\":" + c.winLength + "}";
        }
        if (value instanceof State s) {
            StringBuilder b = new StringBuilder();
            b.append("{\"config\":").append(toJson(s.config));
            b.append(",\"current_player\":\"").append(s.currentPlayer.toWire()).append("\"");
            b.append(",\"board\":[");
            for (int i = 0; i < s.board.size(); i++) {
                if (i > 0) {
                    b.append(',');
                }
                Player cell = s.board.get(i);
                b.append(cell == null ? "null" : "\"" + cell.toWire() + "\"");
            }
            b.append("]}");
            return b.toString();
        }
        if (value instanceof Position p) {
            return "[" + p.row() + "," + p.col() + "]";
        }
        if (value instanceof Player p) {
            return "\"" + p.toWire() + "\"";
        }
        if (value instanceof TttPlayerState ps) {
            return "{\"player\":" + toJson(ps.player) + ",\"state\":" + toJson(ps.state) + "}";
        }
        if (value instanceof MoveEvent e) {
            return "{\"player\":" + toJson(e.player()) + ",\"action\":" + toJson(e.action()) + "}";
        }
        if (value instanceof FullStateWire w) {
            StringBuilder b = new StringBuilder();
            b.append("{\"config\":").append(toJson(w.config()));
            b.append(",\"state\":").append(toJson(w.state()));
            b.append(",\"actions_made\":[");
            for (int i = 0; i < w.actionsMade().size(); i++) {
                if (i > 0) {
                    b.append(',');
                }
                PlayerActionWire a = w.actionsMade().get(i);
                b.append("{\"player\":")
                        .append(toJson(a.player()))
                        .append(",\"action\":")
                        .append(toJson(a.action()))
                        .append('}');
            }
            b.append("]}");
            return b.toString();
        }
        if (value instanceof PlayerActionWire a) {
            return "{\"player\":" + toJson(a.player()) + ",\"action\":" + toJson(a.action()) + "}";
        }
        throw new IllegalArgumentException("unsupported value: " + value.getClass().getName());
    }

    private static Config toConfig(JsonObject o) {
        Config c = new Config();
        c.sideLength = o.intOr("side_length", 3);
        c.winLength = o.intOr("win_length", 3);
        return c;
    }

    private static State toState(JsonObject o) {
        State s = new State();
        s.config = toConfig(o.object("config"));
        s.currentPlayer = Player.fromWire(o.string("current_player"));
        JsonArray board = o.array("board");
        s.board = new ArrayList<>(board.elements.size());
        for (JsonValue cell : board.elements) {
            if (cell instanceof JsonNull) {
                s.board.add(null);
            } else if (cell instanceof JsonString str) {
                s.board.add(Player.fromWire(str.value));
            } else {
                throw new IllegalArgumentException("invalid board cell");
            }
        }
        return s;
    }

    private static Position toPosition(JsonArray a) {
        if (a.elements.size() != 2) {
            throw new IllegalArgumentException("position array length");
        }
        return new Position(a.elements.get(0).asInt(), a.elements.get(1).asInt());
    }

    private static TttPlayerState toTttPlayerState(JsonObject o) {
        return new TttPlayerState(
                Player.fromWire(o.string("player")), toState(o.object("state")));
    }

    private static FullStateWire toFullStateWire(JsonObject o) {
        Config config = toConfig(o.object("config"));
        State state = toState(o.object("state"));
        List<PlayerActionWire> actions = new ArrayList<>();
        for (JsonValue v : o.array("actions_made").elements) {
            JsonObject row = (JsonObject) v;
            actions.add(new PlayerActionWire(
                    Player.fromWire(row.string("player")), toPosition(row.array("action"))));
        }
        return new FullStateWire(config, state, actions);
    }

    private static ParseResult parseValue(String s, int i) {
        i = skipWs(s, i);
        char c = s.charAt(i);
        if (c == '{') {
            return parseObject(s, i);
        }
        if (c == '[') {
            return parseArray(s, i);
        }
        if (c == '"') {
            return parseString(s, i);
        }
        if (c == 'n' && s.startsWith("null", i)) {
            return new ParseResult(new JsonNull(), i + 4);
        }
        if (c == 't' && s.startsWith("true", i)) {
            return new ParseResult(new JsonBool(true), i + 4);
        }
        if (c == 'f' && s.startsWith("false", i)) {
            return new ParseResult(new JsonBool(false), i + 5);
        }
        return parseNumber(s, i);
    }

    private static ParseResult parseObject(String s, int i) {
        i++;
        JsonObject o = new JsonObject();
        i = skipWs(s, i);
        if (s.charAt(i) == '}') {
            return new ParseResult(o, i + 1);
        }
        while (true) {
            ParseResult key = parseString(s, i);
            i = skipWs(s, key.next);
            if (s.charAt(i) != ':') {
                throw new IllegalArgumentException("expected :");
            }
            ParseResult val = parseValue(s, i + 1);
            o.put(((JsonString) key.value).value, val.value);
            i = skipWs(s, val.next);
            char ch = s.charAt(i);
            if (ch == '}') {
                return new ParseResult(o, i + 1);
            }
            if (ch != ',') {
                throw new IllegalArgumentException("expected , or }");
            }
            i = skipWs(s, i + 1);
        }
    }

    private static ParseResult parseArray(String s, int i) {
        i++;
        JsonArray a = new JsonArray();
        i = skipWs(s, i);
        if (s.charAt(i) == ']') {
            return new ParseResult(a, i + 1);
        }
        while (true) {
            ParseResult val = parseValue(s, i);
            a.elements.add(val.value);
            i = skipWs(s, val.next);
            char ch = s.charAt(i);
            if (ch == ']') {
                return new ParseResult(a, i + 1);
            }
            if (ch != ',') {
                throw new IllegalArgumentException("expected , or ]");
            }
            i = skipWs(s, i + 1);
        }
    }

    private static ParseResult parseString(String s, int i) {
        i++;
        StringBuilder b = new StringBuilder();
        while (i < s.length()) {
            char c = s.charAt(i++);
            if (c == '"') {
                return new ParseResult(new JsonString(b.toString()), i);
            }
            if (c == '\\') {
                char esc = s.charAt(i++);
                b.append(
                        switch (esc) {
                            case '"', '\\', '/' -> esc;
                            case 'b' -> '\b';
                            case 'f' -> '\f';
                            case 'n' -> '\n';
                            case 'r' -> '\r';
                            case 't' -> '\t';
                            case 'u' -> {
                                int code =
                                        Integer.parseInt(s.substring(i, i + 4), 16);
                                i += 4;
                                yield (char) code;
                            }
                            default -> throw new IllegalArgumentException("bad escape");
                        });
            } else {
                b.append(c);
            }
        }
        throw new IllegalArgumentException("unterminated string");
    }

    private static ParseResult parseNumber(String s, int i) {
        int start = i;
        if (s.charAt(i) == '-') {
            i++;
        }
        while (i < s.length() && Character.isDigit(s.charAt(i))) {
            i++;
        }
        if (i < s.length() && s.charAt(i) == '.') {
            i++;
            while (i < s.length() && Character.isDigit(s.charAt(i))) {
                i++;
            }
        }
        return new ParseResult(new JsonNumber(s.substring(start, i)), i);
    }

    private static int skipWs(String s, int i) {
        while (i < s.length() && Character.isWhitespace(s.charAt(i))) {
            i++;
        }
        return i;
    }

    private record ParseResult(JsonValue value, int next) {}

    private sealed interface JsonValue permits JsonObject, JsonArray, JsonString, JsonNumber, JsonBool, JsonNull {
        default int asInt() {
            if (this instanceof JsonNumber n) {
                return Integer.parseInt(n.text.split("\\.")[0]);
            }
            throw new IllegalArgumentException("not a number");
        }
    }

    private static final class JsonObject implements JsonValue {
        final java.util.Map<String, JsonValue> fields = new java.util.HashMap<>();

        void put(String k, JsonValue v) {
            fields.put(k, v);
        }

        JsonObject object(String k) {
            return (JsonObject) fields.get(k);
        }

        JsonArray array(String k) {
            return (JsonArray) fields.get(k);
        }

        String string(String k) {
            return ((JsonString) fields.get(k)).value;
        }

        int intOr(String k, int dflt) {
            JsonValue v = fields.get(k);
            return v == null ? dflt : v.asInt();
        }
    }

    private static final class JsonArray implements JsonValue {
        final List<JsonValue> elements = new ArrayList<>();
    }

    private static final class JsonString implements JsonValue {
        final String value;

        JsonString(String value) {
            this.value = value;
        }
    }

    private static final class JsonNumber implements JsonValue {
        final String text;

        JsonNumber(String text) {
            this.text = text;
        }
    }

    private static final class JsonBool implements JsonValue {
        final boolean value;

        JsonBool(boolean value) {
            this.value = value;
        }
    }

    private static final class JsonNull implements JsonValue {}
}
