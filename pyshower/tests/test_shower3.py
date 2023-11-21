import os, sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(
    0, os.path.join(os.path.dirname(__file__), "../lib/python3.11/site-packages")
)
sys.path.insert(
    0, os.path.join(os.path.dirname(__file__), "../lib64/python3.11/site-packages")
)

from pyshower.shower import Shower, ShowerRecordCallback
from time import sleep
import pytest

SQL = "select demo.a from demo limit 10"


class XDemo(ShowerRecordCallback):
    def __init__(self):
        super().__init__()

    def callback(self, data):
        print(data)


class TestShower:
    # 函数级开始
    def setup_method(self):
        os.environ["SHOWER_HOME"] = "/home/helly/code/rust/shower"
        Shower.start()

    # 函数级结束
    def teardown_method(self):
        Shower.stop()

    # 测试
    def test(self):
        callback = XDemo()
        Shower.def_incoming("demo", ["a"], [0], [0])
        global SQL
        mapper_id = Shower.def_mapper(SQL, callback)
        print("mapper_id:", mapper_id)
        futures = list()
        for i in range(100):
            futures.append(Shower.new_data_async(1, b"100000000000000000000000"))
        while True:
            if all(future.done() for future in futures):
                break
            else:
                sleep(0.1)


if __name__ == "__main__":
    pytest.main()
