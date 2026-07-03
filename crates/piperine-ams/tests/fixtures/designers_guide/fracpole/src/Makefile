CSRC	= fracpole.c fp.c ind.c da.c
HSRC	= fracpole.h Macros.h Numbers.h
ARCH	= 
CC	= gcc
CFLAGS	= -g

OBJ	= $(CSRC:.c=.o)

all	: fp ind da

fp	: $(OBJ)
	rm -f fp
	$(CC) fp.o fracpole.o $(ARCH) -lm -o fp

ind	: $(OBJ)
	rm -f ind
	$(CC) ind.o fracpole.o $(ARCH) -lm -o ind

da	: $(OBJ)
	rm -f da
	$(CC) da.o fracpole.o $(ARCH) -lm -o da

install	: all
	cp fp ind da ../bin

$(OBJ)	: $(HSRC) Makefile

listcsrc:
	@echo $(CSRC)

listhsrc:
	@echo $(HSRC)

listobj:
	@echo $(OBJ)

clean:
	@rm -f $(OBJ) $(DEST)
