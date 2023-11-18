package com.erayt.shower4j;

import org.openjdk.jmh.annotations.Benchmark;
import org.openjdk.jmh.annotations.Fork;
import org.openjdk.jmh.annotations.Measurement;
import org.openjdk.jmh.annotations.OutputTimeUnit;
import org.openjdk.jmh.annotations.Scope;
import org.openjdk.jmh.annotations.Setup;
import org.openjdk.jmh.annotations.State;
import org.openjdk.jmh.annotations.TearDown;
import org.openjdk.jmh.annotations.Threads;
import org.openjdk.jmh.annotations.Warmup;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;

/**
 * @author Helly Guo
 * <p>
 * Created on 10/25/23 2:10 PM
 */

@State(Scope.Benchmark)
@Fork(value = 3)
@Threads(value = 1)
@Warmup(iterations = 10, time = 1)
@Measurement(iterations = 20, time = 1)
@OutputTimeUnit(TimeUnit.MICROSECONDS)
// TODO: need to fix it
public class ShowerBenchmark {

    private static final String SQL = "select demo.a, demo.b, demo.c, demo.d from demo limit 10";
    private static final String SQL2 = "select _suml(stream.a) from stream";

    private static final SimpleData DATA = new SimpleData(1, 2L, 3.45D, "hello");

    private static int targetId;

    @Setup
    public static void setUp() {
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
            System.exit(1);
        }
        int recordId2 = JavaShower.defStream("stream", list2);
        if (recordId2 == -1) {
            System.exit(1);
        }
        int aggregateId = JavaShower.defAggregate(SQL2, (data, size) -> {
        });
        if (aggregateId == -1) {
            System.exit(1);
        }
        int mapperId = JavaShower.defMapperBindAggregate(SQL, aggregateId);
        if (mapperId == -1) {
            System.exit(1);
        }
        JavaShower.regConvert(recordId, converter);
        targetId = recordId;
    }

    @TearDown
    public static void tearDown() {
        JavaShower.stop();
    }

    @Benchmark
    public void test() {
        JavaShower.newData(targetId, DATA);
    }
}

