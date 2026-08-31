package cn.itcraft.bpe4j;

import org.openjdk.jmh.runner.Runner;
import org.openjdk.jmh.runner.options.OptionsBuilder;
import org.openjdk.jmh.runner.options.TimeValue;

/**
 * Entry point for the average-position (last-N trades) benchmark.
 * Run with: java -Djava.library.path=<path-to-release-so> ... JmhPositionAvgBenchMain
 */
public final class JmhPositionAvgBenchMain {
    public static void main(String[] args) throws Exception {
        org.openjdk.jmh.runner.options.Options opt = new OptionsBuilder()
                .include(PositionAvgBench.class.getSimpleName())
                .shouldFailOnError(true)
                .warmupIterations(2).warmupTime(TimeValue.seconds(8))
                .measurementIterations(4).measurementTime(TimeValue.seconds(8))
                .build();
        new Runner(opt).run();
    }

    private JmhPositionAvgBenchMain() {
    }
}
