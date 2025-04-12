package com.erayt.bambootube4j;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;

/**
 * learn from <a href="https://www.cnblogs.com/FlyingPuPu/p/7598098.html">load from jar</a><br/>
 *
 * @author Helly Guo
 * <p>
 * Created on 11/16/23 10:23 PM
 */
class NativeLoader {

    private static final Logger LOGGER = LoggerFactory.getLogger(NativeLoader.class);

    private static final String TMP_DIR = System.getProperty("java.io.tmpdir");

    /**
     * identify native library by system, may be need "os.arch"
     */
    private static final String OS_NAME = System.getProperty("os.name");
    private static final String EXT = (OS_NAME.toLowerCase().contains("win")) ? ".dll" : ".so";

    private static final String BAMBOOTUBE_LIB_SHORT_NAME = "bambootube4j";
    private static final String BAMBOOTUBE_LIB_PREFIX = "lib" + BAMBOOTUBE_LIB_SHORT_NAME;
    private static final String BAMBOOTUBE_LIB_NAME = BAMBOOTUBE_LIB_PREFIX + EXT;
    private static final String BAMBOOTUBE_LIB_IN_JAR_PATH = "resources/" + BAMBOOTUBE_LIB_NAME;

    private static final String BAMBOOTUBE_LIB_DEF = "ENV_LIB_PARAM_NOT_EXIST";
    private static final String BAMBOOTUBE_LIB = System.getProperty("bambootubeLib", BAMBOOTUBE_LIB_DEF);

    public static void load() {
        LOGGER.info("try load native library[{}] from sys lib path", BAMBOOTUBE_LIB_NAME);
        try {
            System.loadLibrary(BAMBOOTUBE_LIB_SHORT_NAME);
            return;
        } catch (UnsatisfiedLinkError error) {
            LOGGER.warn("try load lib from sys lib path failed: {}", error.getMessage());
        }
        if (BAMBOOTUBE_LIB_DEF.equals(BAMBOOTUBE_LIB)) {
            LOGGER.info("try load native library[{}] from classpath", BAMBOOTUBE_LIB_IN_JAR_PATH);
            loadFromJar();
        } else {
            LOGGER.info("try load native library[{}] from {}", BAMBOOTUBE_LIB_NAME, BAMBOOTUBE_LIB);
            loadFromSysProperties();
        }
        LOGGER.info("load native library[{}] success", BAMBOOTUBE_LIB_NAME);
    }

    private static void loadFromJar() {
        try (InputStream is = Thread.currentThread()
                .getContextClassLoader().getResourceAsStream(BAMBOOTUBE_LIB_IN_JAR_PATH)) {
            if (is == null) {
                throw new RuntimeException(BAMBOOTUBE_LIB_IN_JAR_PATH + " is not found in classpath");
            }
            Path tmpDir = Paths.get(TMP_DIR);
            Path tmpLib = Files.createTempFile(tmpDir, BAMBOOTUBE_LIB_PREFIX, EXT);
            tmpLib.toFile().deleteOnExit();
            Files.copy(is, tmpLib, StandardCopyOption.REPLACE_EXISTING);
            System.load(tmpLib.toAbsolutePath().toString());
        } catch (UnsatisfiedLinkError | IOException e) {
            LOGGER.warn("failed to load native library[{}] from classpath: {}", BAMBOOTUBE_LIB_IN_JAR_PATH, e.getMessage());
            throw new RuntimeException(e);
        }
    }

    private static void loadFromSysProperties() {
        try {
            System.load(BAMBOOTUBE_LIB);
        } catch (UnsatisfiedLinkError e) {
            LOGGER.info("failed to load native library[{}] from {}: {}", BAMBOOTUBE_LIB_NAME, BAMBOOTUBE_LIB, e.getMessage());
            throw new RuntimeException(e);
        }
    }
}
