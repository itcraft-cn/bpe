package cn.itcraft.bpe4j;

import org.openjdk.jmh.annotations.*;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.LongAdder;

/**
 * Simulated business scenario: average position of the last N trades (N <= 5).
 *
 * Trades flow into BPE; a mapper keeps the newest N records (LIMIT N) and a
 * bound aggregate computes AVG(signed_qty) over them; after EVERY trade the
 * average is delivered to the Java-side callback (on the egress thread).
 *
 * Throughput accounting (per-trade confirmation):
 * - each invocation fires OPS trades without waiting for per-trade acks;
 * - each aggregate callback (one per trade) increments a LongAdder;
 * - the invocation returns only after all OPS trades are confirmed.
 *
 * The LIMIT N parameter sweeps the sliding-window depth: 1, 3, 5.
 */
@BenchmarkMode(Mode.Throughput)
@OutputTimeUnit(TimeUnit.SECONDS)
@State(Scope.Benchmark)
@Fork(1)
@Warmup(iterations = 2, time = 8)
@Measurement(iterations = 4, time = 8)
public class PositionAvgBench {

    private static final int OPS = 100_000;
    private static final long REPORT_THRESHOLD = 100_000L;
    private static final long AWAIT_TIMEOUT_MS = 30_000L;

    @Param({"1", "3", "5"})
    int limitN;

    private int recordId;
    private long lcg;
    /** trades submitted (fire-and-forget) */
    private final AtomicLong submitted = new AtomicLong();
    /** trades processed, confirmed by per-trade aggregate callbacks */
    private final LongAdder processed = new LongAdder();
    private volatile long nextReport = REPORT_THRESHOLD;
    private volatile double lastAvg;
    /** sanity: count of callbacks whose avg looks sane (|avg| <= 500k * N) */
    private final AtomicLong saneAvg = new AtomicLong();

    @Setup
    public void setup() {
        JavaBpe.start();
        // mapper stream: qty must match the aggregate stream field name so the
        // bound aggregate resolves its input offset by name
        List<ColumnDefine> schema = new ArrayList<>();
        schema.add(ColumnDefine.createLong("qty"));
        schema.add(ColumnDefine.createLong("ts"));
        recordId = JavaBpe.defIncoming("benchavg", schema);
        if (recordId == -1) {
            throw new IllegalStateException("defIncoming failed");
        }
        List<ColumnDefine> aggSchema = new ArrayList<>();
        aggSchema.add(ColumnDefine.createLong("qty"));
        aggSchema.add(ColumnDefine.createLong("ts"));
        int streamId = JavaBpe.defStream("avgstream", aggSchema);
        if (streamId == -1) {
            throw new IllegalStateException("defStream failed");
        }
        JavaBpe.regConvert(recordId, new TradeConverter());

        int aggId = JavaBpe.defAggregate(
                "select _avg(avgstream.qty) from avgstream",
                (data, size) -> {
                    if (size >= 1) {
                        double avg = ByteBuffer.wrap(data, 0, 8)
                                .order(ByteOrder.LITTLE_ENDIAN).getDouble(0);
                        lastAvg = avg;
                        if (Math.abs(avg) <= 500_000.0 * limitN) {
                            saneAvg.incrementAndGet();
                        }
                        processed.increment();
                        long done = processed.sum();
                        if (done >= nextReport) {
                            System.out.printf("[report] n=%d processed=%d submitted=%d lastAvg=%.1f%n",
                                    limitN, done, submitted.get(), avg);
                            nextReport += REPORT_THRESHOLD;
                        }
                    }
                });
        if (aggId == -1) {
            throw new IllegalStateException("defAggregate failed");
        }
        String mapperSql = "select benchavg.qty, benchavg.ts from benchavg limit " + limitN;
        int mapperId = JavaBpe.defMapperBindAggregate(mapperSql, aggId);
        if (mapperId == -1) {
            throw new IllegalStateException("defMapperBindAggregate failed, sql: " + mapperSql);
        }
        lcg = 42;
    }

    /**
     * One invocation = OPS trades: fire-and-forget submission, then wait until
     * every trade's average-position callback has been delivered.
     */
    @Benchmark
    @OperationsPerInvocation(OPS)
    public void trades() throws Exception {
        final long target = submitted.get() + OPS;
        for (int i = 0; i < OPS; i++) {
            Trade t = new Trade();
            t.signedQty = nextQty();
            t.ts = System.currentTimeMillis();
            JavaBpe.newDataAsync(recordId, t);
            submitted.incrementAndGet();
        }
        awaitProcessed(target);
    }

    private void awaitProcessed(long target) throws InterruptedException {
        long deadline = System.currentTimeMillis() + AWAIT_TIMEOUT_MS;
        while (processed.sum() < target) {
            if (System.currentTimeMillis() > deadline) {
                throw new IllegalStateException(
                        "processing timeout: processed=" + processed.sum() + " target=" + target);
            }
            Thread.sleep(1);
        }
    }

    /** Deterministic pseudo-random net-long flow: +100k..+500k (always long). */
    private long nextQty() {
        lcg = lcg * 6364136223846793005L + 1442695040888963407L;
        return 100_000L + ((lcg >>> 33) % 400_000L);
    }

    @TearDown
    public void tearDown() throws InterruptedException {
        Thread.sleep(200); // let the egress channel drain the last callbacks
        System.out.println();
        System.out.println("==== avg position of last N trades (n=" + limitN + ") ====");
        System.out.println("trades submitted  : " + submitted.get());
        System.out.println("trades processed  : " + processed.sum() + " (per-trade callback confirmed)");
        System.out.println("sane avg callbacks: " + saneAvg.get());
        System.out.println("last avg position : " + lastAvg);
        System.out.println("egress pending    : " + JavaBpe.pendingEvents());
        System.out.println("egress dropped    : " + JavaBpe.droppedEvents());
        System.out.println("egress alerts(wd) : " + JavaBpe.alertCount());
        JavaBpe.stop();
    }

    /** Trade record: [qty i64 @0][ts i64 @8] in the 512-byte slot. */
    static final class Trade {
        long signedQty;
        long ts;
    }

    static final class TradeConverter implements ByteConverter<Trade> {
        @Override
        public byte[] convert(Trade data) {
            byte[] rec = new byte[512];
            ByteBuffer buf = ByteBuffer.wrap(rec).order(ByteOrder.LITTLE_ENDIAN);
            buf.putLong(0, data.signedQty);
            buf.putLong(8, data.ts);
            return rec;
        }

        @Override
        public Trade convert(byte[] data, int offset) {
            Trade t = new Trade();
            ByteBuffer buf = ByteBuffer.wrap(data, offset, 16).order(ByteOrder.LITTLE_ENDIAN);
            t.signedQty = buf.getLong(0);
            t.ts = buf.getLong(8);
            return t;
        }
    }
}
