import os

# Since mechaphlowers enables the mphlowrs kernels at import (`rust` extra), keep its
# pure Python implementations as the reference: tests plug the kernels in explicitly.
os.environ["MECHAPHLOWERS_RUST"] = "0"
