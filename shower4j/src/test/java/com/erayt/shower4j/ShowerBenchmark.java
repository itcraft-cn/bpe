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

import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;

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
public class ShowerBenchmark {
    private static final String SQL = "select _1.__1 from _1 limit 1";
    private static final byte[] DATA = {
            0xf, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
    };

    @Setup
    public static void setUp() {
        AtomicLong val = new AtomicLong(0);
        Shower.start();
        if (!Shower.defActionWithCallback(SQL, (data, size) -> val.getAndAdd(size))) {
            System.exit(1);
        }
    }

    @TearDown
    public static void tearDown() {
        Shower.stop();
    }

    @Benchmark
    public void test() {
        Shower.newData(1, DATA);
    }
}

