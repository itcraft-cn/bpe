package com.erayt.shower4j;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Scanner;

/**
 * @author Helly Guo
 * <p>
 * Created on 10/25/23 2:10 PM
 */
// TODO: need to fix it
public class ShowerSample {
    private static final Logger LOGGER = LoggerFactory.getLogger(ShowerSample.class);

    private static final String SQL = "select _1.__1 from _1 limit 1";

    private static final byte[] DATA = {
            1, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
    };

    private static final int LOOP_SIZE = 10000000;

    public static void main(String[] args) {
        Shower.start();
        if (Shower.defMapper(SQL, ShowerSample::listenData) == -1) {
            LOGGER.warn("failed to def action");
        } else {
            waitCmd();
        }
        Shower.stop();
    }

    private static void listenData(byte[] data, int size) {
    }

    private static void waitCmd() {
        Scanner scanner = new Scanner(System.in);
        String line;
        while (true) {
            LOGGER.info("waiting for input:");
            line = scanner.nextLine();
            if ("exit".equals(line)) {
                break;
            } else if ("run".equals(line)) {
                sendData();
            }
        }
        LOGGER.info("ready to quit");
    }

    private static void sendData() {
        long start = System.nanoTime();
        boolean success;
        for (int i = 0; i < LOOP_SIZE; i++) {
            success = Shower.newData(1, DATA);
            if (!success) {
                LOGGER.warn("failed to send data");
                break;
            }
        }
        long end = System.nanoTime();
        LOGGER.info("send {} data in {} ms, {} ns", LOOP_SIZE, (end - start) / 1000D / 1000D, end - start);
    }
}
