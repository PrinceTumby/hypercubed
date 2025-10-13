LDFLAGS += -L../../opt/w2c2/lib -lw2c2wasi
LDFLAGS += -lm -lobjc
LDFLAGS += -framework Cocoa -framework OpenGL -framework Quartz
CFLAGS += -O1 -g

# hypercubed: main.c hypercubed.c
# 	$(CC) -I../../opt/w2c2/include $(CFLAGS) $^ -o $@ $(LDFLAGS)

EXTRASOURCES = $(wildcard s*.c)
EXTRAOBJECTS = $(EXTRASOURCES:.c=.o)

# `build.rs` generated bindings for W2C2.
BINDING_OBJS = gl.o

OBJECTS = $(BINDING_OBJS) main.o hypercubed.o $(EXTRAOBJECTS)

hypercubed.app/Contents/MacOS/hypercubed: $(OBJECTS)
	$(CC) $^ -o $@ $(LDFLAGS) -sectcreate __DATA __datasegments datasegments

hypercubed.o: hypercubed.c
	$(CC) -I../../opt/w2c2/include -c -O1 $< -o $@

%.o: %.c
	$(CC) -I../../opt/w2c2/include -c $(CFLAGS) $< -o $@

%.o: %.m
	$(CC) -I../../opt/w2c2/include -c $(CFLAGS) $< -o $@
