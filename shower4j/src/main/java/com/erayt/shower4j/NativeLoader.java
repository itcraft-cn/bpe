package com.erayt.shower4j;

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

    private static final String SHOWER_LIB_SHORT_NAME = "shower4j";
    private static final String SHOWER_LIB_PREFIX = "lib" + SHOWER_LIB_SHORT_NAME;
    private static final String SHOWER_LIB_NAME = SHOWER_LIB_PREFIX + EXT;
    private static final String SHOWER_LIB_IN_JAR_PATH = "resources/" + SHOWER_LIB_NAME;

    private static final String SHOWER_LIB_DEF = "ENV_LIB_PARAM_NOT_EXIST";
    private static final String SHOWER_LIB = System.getProperty("showerLib", SHOWER_LIB_DEF);

    public static void load() {
        LOGGER.info("try load native library[{}] from sys lib path", SHOWER_LIB_NAME);
        try {
            System.loadLibrary(SHOWER_LIB_SHORT_NAME);
            return;
        } catch (UnsatisfiedLinkError error) {
            LOGGER.warn("try load lib from sys lib path failed: {}", error.getMessage());
        }
        if (SHOWER_LIB_DEF.equals(SHOWER_LIB)) {
            LOGGER.info("try load native library[{}] from classpath", SHOWER_LIB_IN_JAR_PATH);
            loadFromJar();
        } else {
            LOGGER.info("try load native library[{}] from {}", SHOWER_LIB_NAME, SHOWER_LIB);
            loadFromSysProperties();
        }
        LOGGER.info("load native library[{}] success", SHOWER_LIB_NAME);
    }

    private static void loadFromJar() {
        try (InputStream is = Thread.currentThread()
                .getContextClassLoader().getResourceAsStream(SHOWER_LIB_IN_JAR_PATH)) {
            if (is == null) {
                throw new RuntimeException(SHOWER_LIB_IN_JAR_PATH + " is not found in classpath");
            }
            Path tmpDir = Paths.get(TMP_DIR);
            Path tmpLib = Files.createTempFile(tmpDir, SHOWER_LIB_PREFIX, EXT);
            tmpLib.toFile().deleteOnExit();
            Files.copy(is, tmpLib, StandardCopyOption.REPLACE_EXISTING);
            System.load(tmpLib.toAbsolutePath().toString());
        } catch (UnsatisfiedLinkError | IOException e) {
            LOGGER.warn("failed to load native library[{}] from classpath: {}", SHOWER_LIB_IN_JAR_PATH, e.getMessage());
            throw new RuntimeException(e);
        }
    }

    private static void loadFromSysProperties() {
        try {
            System.load(SHOWER_LIB);
        } catch (UnsatisfiedLinkError e) {
            LOGGER.info("failed to load native library[{}] from {}: {}", SHOWER_LIB_NAME, SHOWER_LIB, e.getMessage());
            throw new RuntimeException(e);
        }
    }
}
