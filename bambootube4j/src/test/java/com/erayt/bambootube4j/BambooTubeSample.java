package com.erayt.bambootube4j;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.List;
import java.util.Scanner;
import java.util.concurrent.atomic.AtomicLong;

/**
 * @author Helly Guo
 * <p>
 * Created on 10/25/23 2:10 PM
 */
public class BambooTubeSample {
    private static final Logger LOGGER = LoggerFactory.getLogger(BambooTubeSample.class);

    private static final String SQL = "select demo.a, demo.b, demo.c, demo.d from demo limit 10";
    private static final String SQL2 = "select _suml(stream.a),_sumd(stream.c) from stream";

    private static final SimpleData DATA = new SimpleData(1, 2L, 3.45D, "hello");

    private static final int LOOP_SIZE = 10000000;

    private static final SimpleDataConverter CONVERTER = new SimpleDataConverter();

    private static final AtomicLong COUNTER = new AtomicLong(0);
    private static final AtomicLong DATA1_SUM = new AtomicLong(0);
    private static final AtomicLong DATA2_SUM = new AtomicLong(0);
    private static final AtomicLong DATA3_SUM = new AtomicLong(0);

    public static void main(String[] args) {
        JavaBambooTube.start();
        define();
        waitCmd();
        JavaBambooTube.stop();
    }

    private static void listenData(byte[] data, int size) {
        COUNTER.incrementAndGet();
        for (int i = 0; i < size; i++) {
            SimpleData converted = CONVERTER.convert(data, i * 512);
            DATA1_SUM.addAndGet(converted.getVal1());
            DATA2_SUM.addAndGet(converted.getVal2());
            double v = converted.getVal3();
            double sum = Double.longBitsToDouble(DATA3_SUM.get());
            sum += v;
            DATA3_SUM.set(Double.doubleToLongBits(sum));
        }
    }

    private static void waitCmd() {
        Scanner scanner = new Scanner(System.in);
        String line;
        while (true) {
            LOGGER.info("waiting for input(run|exit) :");
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
            success = JavaBambooTube.newData(1, DATA);
            if (!success) {
                LOGGER.warn("failed to send data");
                break;
            }
        }
        long end = System.nanoTime();
        long nsCost = end - start;
        double msCost = nsCost / 1000D / 1000D;
        LOGGER.info("sum: send {} times in {} ms, {} ns", LOOP_SIZE, msCost, nsCost);
        LOGGER.info("avg: send {} times in {} ms, {} ns", LOOP_SIZE, msCost / LOOP_SIZE, nsCost / LOOP_SIZE);
        LOGGER.info("output: counter={}", COUNTER.get());
        LOGGER.info("output: sum1={}", DATA1_SUM.get());
        LOGGER.info("output: sum2={}", DATA2_SUM.get());
        LOGGER.info("output: sum3={}", DATA3_SUM.get());
    }

    private static void define() {
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
        int recordId = JavaBambooTube.defIncoming("demo", list);
        if (recordId == -1) {
            LOGGER.warn("failed to def record");
            throw new RuntimeException("failed to def record");
        }
        int recordId2 = JavaBambooTube.defStream("stream", list2);
        if (recordId2 == -1) {
            LOGGER.warn("failed to def stream");
            throw new RuntimeException("failed to def stream");
        }
        int aggregateId = JavaBambooTube.defAggregate(SQL2, BambooTubeSample::listenData);
        if (aggregateId == -1) {
            LOGGER.warn("failed to def aggregate");
            throw new RuntimeException("failed to def aggregate");
        }
        int mapperId = JavaBambooTube.defMapperBindAggregate(SQL, aggregateId);
        if (mapperId == -1) {
            LOGGER.warn("failed to def mapper");
            throw new RuntimeException("failed to def mapper");
        }
        JavaBambooTube.regConvert(recordId, CONVERTER);
    }
}
