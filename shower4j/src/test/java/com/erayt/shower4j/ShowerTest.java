package com.erayt.shower4j;

import org.junit.Test;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * @author Helly Guo
 * <p>
 * Created on 10/25/23 2:10 PM
 */
public class ShowerTest {
    private static final Logger LOGGER = LoggerFactory.getLogger(ShowerTest.class);
    private static final String SQL = "select _1.__1 from _1 limit 1";

    @Test
    public void test() {
        Shower.start();
        if (Shower.defActionWithCallback(SQL, (data, size) -> LOGGER.info("{}, {}", data, size))) {
            for (int i = 0; i < 100; i++) {
                byte b = (byte) (i % 256);
                Shower.newData(1,
                        new byte[]{
                                b, 0, 0, 0, 0, 0, 0, 0,
                                0, 0, 0, 0, 0, 0, 0, 0,
                                0, 0, 0, 0, 0, 0, 0, 0,
                                0, 0, 0, 0, 0, 0, 0, 0,
                                0, 0, 0, 0, 0, 0, 0, 0,
                                0, 0, 0, 0, 0, 0, 0, 0,
                                0, 0, 0, 0, 0, 0, 0, 0,
                                0, 0, 0, 0, 0, 0, 0, 0,
                        });
            }
        } else {
            LOGGER.warn("failed to def action");
        }
        Shower.stop();
    }
}
