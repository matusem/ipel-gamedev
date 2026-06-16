package sk.upjs.gdd.game.tooling;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Writes a minimal JSON Schema IR bundle for client codegen ({@code gamedev codegen}).
 * Game projects extend/replace types in their own {@code game} module and re-run export.
 */
public final class ExportJsonSchema {
    private ExportJsonSchema() {}

    public static void main(String[] args) throws Exception {
        Path outDir = Path.of(args.length > 0 ? args[0] : "build/schema");
        Files.createDirectories(outDir);
        ObjectMapper mapper = new ObjectMapper();
        ObjectNode root = mapper.createObjectNode();
        root.put("$schema", "http://json-schema.org/draft-07/schema#");
        root.put("title", "GameTypes");
        ObjectNode defs = root.putObject("definitions");
        defs.putObject("Player").put("type", "string");
        ObjectNode config = defs.putObject("Config");
        config.put("type", "object");
        ObjectNode configProps = config.putObject("properties");
        configProps.putObject("side_length").put("type", "integer");
        configProps.putObject("win_length").put("type", "integer");
        Path out = outDir.resolve("game-types.json");
        mapper.writerWithDefaultPrettyPrinter().writeValue(out.toFile(), root);
        System.out.println("Wrote " + out.toAbsolutePath());
    }
}
