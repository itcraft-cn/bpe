package com.erayt.shower4j;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.List;
import java.util.Scanner;

/**
 * @author Helly Guo
 * <p>
 * Created on 10/25/23 2:10 PM
 */
// TODO: need to fix it
public class ShowerSample {
    private static final Logger LOGGER = LoggerFactory.getLogger(ShowerSample.class);

    private static final String SQL = "select demo.a, demo.b, demo.c, demo.d from demo limit 10";
    private static final String SQL2 = "select _suml(stream.a) from stream";

    private static final SimpleData DATA = new SimpleData(1, 2L, 3.45D, "hello");

    private static final int LOOP_SIZE = 10000000;

    public static void main(String[] args) {
        waitCmd();
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
        JavaShower.start();
        SimpleDataConverter converter = new SimpleDataConverter();
        List<ColumnDefine> list = new ArrayList<>();
        list.add(ColumnDefine.createLong("a"));
        list.add(ColumnDefine.createLong("b"));
        list.add(ColumnDefine.createDouble("c"));
        list.add(ColumnDefine.createString("d", 16));
        List<ColumnDefine> list2 = new ArrayList<>();
        list2.add(ColumnDefine.createLong("a"));
        list2.add(ColumnDefine.createLong("b"));
        list2.add(ColumnDefine.createDouble("c"));
        list2.add(ColumnDefine.createString("d", 16));
        int recordId = JavaShower.defIncoming("demo", list);
        if (recordId == -1) {
            LOGGER.warn("failed to def record");
            return;
        }
        int recordId2 = JavaShower.defStream("stream", list2);
        if (recordId2 == -1) {
            LOGGER.warn("failed to def record");
            return;
        }
        int aggregateId = JavaShower.defAggregate(SQL2, ShowerSample::listenData);
        if (aggregateId == -1) {
            LOGGER.warn("failed to def aggregate");
            return;
        }
        int mapperId = JavaShower.defMapperBindAggregate(SQL, aggregateId);
        if (mapperId == -1) {
            LOGGER.warn("failed to def mapper");
            return;
        }
        JavaShower.regConvert(recordId, converter);
        long start = System.nanoTime();
        boolean success;
        for (int i = 0; i < LOOP_SIZE; i++) {
            success = JavaShower.newData(1, DATA);
            if (!success) {
                LOGGER.warn("failed to send data");
                break;
            }
        }
        long end = System.nanoTime();
        LOGGER.info("send {} data in {} ms, {} ns", LOOP_SIZE, (end - start) / 1000D / 1000D, end - start);
        JavaShower.stop();
    }
}
