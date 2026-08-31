package cn.itcraft.bpe4j;

import java.util.concurrent.TimeUnit;
import org.openjdk.jmh.runner.Runner;
import org.openjdk.jmh.runner.options.TimeValue;
import org.openjdk.jmh.runner.options.OptionsBuilder;

/**
 * Entry point for the position-accumulation benchmark.
 * Run with: java -Djava.library.path=<path-to-release-so> ... JmhPositionBenchMain
 */
public final class JmhPositionBenchMain {
    public static void main(String[] args) throws Exception {
        org.openjdk.jmh.runner.options.Options opt = new OptionsBuilder()
                .include(PositionWindowBench.class.getSimpleName())
                .warmupIterations(3).warmupTime(TimeValue.seconds(2))
                .measurementIterations(6).measurementTime(TimeValue.seconds(2))
                .shouldFailOnError(true)
                .jvmArgsAppend("-Djava.library.path=" + System.getProperty("java.library.path"))
                .build();
        new Runner(opt).run();
    }

    private JmhPositionBenchMain() {
    }
}
